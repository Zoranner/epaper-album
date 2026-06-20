mod auth;
mod images;
mod plans;
mod sprites;

use std::{collections::HashSet, sync::Arc};

use axum::{
    routing::{get, post, put},
    Router,
};
use tokio::sync::{mpsc, Mutex};
use tower_http::services::ServeDir;

use crate::{
    error::AppError,
    files::{display_image_path, display_image_temp_path, find_original_image_path},
    graphics::images::render_panel_bmp,
    state::{AppState, ProcessingQueue, RuntimeState},
};

pub fn router(state: AppState) -> Router {
    let runtime = RuntimeState {
        queue: state
            .enqueue_processing
            .then(|| start_processing_worker(state.clone())),
        app: state,
    };
    spawn_pending_enqueue(&runtime);

    Router::new()
        .route("/api/healthz", get(auth::healthz))
        .route("/api/login", post(auth::login))
        .route(
            "/api/plans",
            get(plans::list_plans).post(plans::create_plan),
        )
        .route(
            "/api/plans/:date",
            put(plans::update_plan).delete(plans::delete_plan),
        )
        .route(
            "/api/images",
            get(images::list_images).post(images::upload_image),
        )
        .route(
            "/api/images/:sha256",
            put(images::update_image).delete(images::delete_image),
        )
        .route("/api/images/:sha256/redither", post(images::redither_image))
        .route("/api/sprites", get(sprites::sprite_metadata))
        .route("/images/:sha256", get(images::download_image))
        .route("/sprites/:sha256", get(sprites::download_sprite))
        .fallback_service(ServeDir::new("web/dist").append_index_html_on_directories(true))
        .with_state(runtime)
}

pub async fn recover_and_enqueue_pending(state: &AppState) -> anyhow::Result<()> {
    state.store.recover_processing_images().await?;

    let ready_sha256s = state.store.ready_sha256s().await?;
    let missing = ready_sha256s
        .into_iter()
        .filter(|sha256| !display_image_path(&state.data_dir, sha256).exists())
        .collect::<Vec<_>>();
    state
        .store
        .mark_ready_missing_display_pending(&missing)
        .await?;

    Ok(())
}

fn spawn_pending_enqueue(runtime: &RuntimeState) {
    let Some(queue) = runtime.queue.clone() else {
        return;
    };
    let store = runtime.app.store.clone();
    tokio::spawn(async move {
        match store.pending_sha256s().await {
            Ok(sha256s) => {
                for sha256 in sha256s {
                    let mut queued = queue.queued.lock().await;
                    if queued.insert(sha256.clone()) && queue.sender.send(sha256.clone()).is_err() {
                        queued.remove(&sha256);
                    }
                }
            }
            Err(error) => tracing::error!("failed to enqueue pending images: {error:?}"),
        }
    });
}

pub(super) async fn enqueue_image(state: &RuntimeState, sha256: String) {
    let Some(queue) = &state.queue else {
        return;
    };

    let mut queued = queue.queued.lock().await;
    if queued.insert(sha256.clone()) && queue.sender.send(sha256.clone()).is_err() {
        queued.remove(&sha256);
    }
}

fn start_processing_worker(state: AppState) -> ProcessingQueue {
    let (sender, mut receiver) = mpsc::unbounded_channel::<String>();
    let queued = Arc::new(Mutex::new(HashSet::new()));
    let worker_queued = queued.clone();

    tokio::spawn(async move {
        while let Some(sha256) = receiver.recv().await {
            {
                let mut queued = worker_queued.lock().await;
                queued.remove(&sha256);
            }

            if let Err(error) = process_one_image(&state, &sha256).await {
                tracing::error!("image processing failed for {sha256}: {error:?}");
                if let Err(mark_error) = state.store.mark_failed(&sha256).await {
                    tracing::error!("failed to mark image failed for {sha256}: {mark_error:?}");
                }
            }
        }
    });

    ProcessingQueue { sender, queued }
}

async fn process_one_image(state: &AppState, sha256: &str) -> anyhow::Result<()> {
    if !state.store.claim_pending(sha256).await? {
        return Ok(());
    }

    let result = async {
        let original_path = find_original_image_path(&state.data_dir, sha256)?;
        let display_path = display_image_path(&state.data_dir, sha256);
        let temp_path = display_image_temp_path(&state.data_dir, sha256);
        let bytes = tokio::fs::read(original_path).await?;
        let bmp = tokio::task::spawn_blocking(move || render_panel_bmp(&bytes)).await??;
        tokio::fs::write(&temp_path, bmp).await?;
        if display_path.exists() {
            tokio::fs::remove_file(&display_path).await?;
        }
        tokio::fs::rename(&temp_path, &display_path).await?;
        anyhow::Ok(())
    }
    .await;

    match result {
        Ok(()) => state.store.mark_ready(sha256).await?,
        Err(error) => {
            state.store.mark_failed(sha256).await?;
            return Err(error);
        }
    }

    Ok(())
}

pub(super) fn validate_sha256(value: &str) -> Result<(), AppError> {
    let valid = value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit());
    if valid {
        Ok(())
    } else {
        Err(AppError::BadRequest("图片标识格式不正确".to_string()))
    }
}
