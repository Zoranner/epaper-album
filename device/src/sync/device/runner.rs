use super::cache::cached_image_is_renderable;
use super::DeviceSyncError;
use crate::cloud::HttpClient;
use crate::device_runtime::{DeviceCloudSync, SpriteSet, SyncErrorReport, SyncRequest, SyncResult};
use crate::resource_sync::{download_image, download_plan, sync_sprite};
use crate::storage::ResourceStore;

pub const PLAN_LOOKAHEAD_DAYS: u8 = 3;

pub struct CloudResourceSync<C, S> {
    client: C,
    store: S,
    days: u8,
}

impl<C, S> CloudResourceSync<C, S> {
    pub const fn new(client: C, store: S) -> Self {
        Self {
            client,
            store,
            days: PLAN_LOOKAHEAD_DAYS,
        }
    }

    pub const fn with_days(mut self, days: u8) -> Self {
        self.days = days;
        self
    }

    pub fn into_parts(self) -> (C, S) {
        (self.client, self.store)
    }
}

impl<C, S> DeviceCloudSync for CloudResourceSync<C, S>
where
    C: HttpClient,
    S: ResourceStore,
{
    type Error = DeviceSyncError;

    fn sync_resources(&mut self, request: SyncRequest) -> Result<SyncResult, Self::Error> {
        let base_url = request.config.base_url.trim();
        let secret_key = request.config.secret_key.trim();
        let plans = download_plan(
            &mut self.client,
            &mut self.store,
            base_url,
            secret_key,
            self.days,
        )
        .map_err(|error| DeviceSyncError::resource("plan", error))?;
        let mut sprites = SpriteSet::default();

        for plan in &plans {
            if cached_image_is_renderable(&self.store, &plan.image) {
                continue;
            }
            download_image(
                &mut self.client,
                &mut self.store,
                base_url,
                secret_key,
                &plan.image,
            )
            .map_err(|error| DeviceSyncError::resource(format!("image {}", plan.image), error))?;
        }

        if let Some(plan) = crate::schedule::select_plan_for_date(&plans, request.date) {
            if !plan.caption.trim().is_empty() {
                let result = sync_sprite(
                    &mut self.client,
                    &mut self.store,
                    base_url,
                    secret_key,
                    "caption",
                    &plan.caption,
                )
                .map_err(|error| {
                    DeviceSyncError::resource(format!("sprite caption {}", plan.caption), error)
                })?;
                sprites.caption = Some(result.sha256);
            }
        }
        let result = sync_sprite(
            &mut self.client,
            &mut self.store,
            base_url,
            secret_key,
            "date",
            &request.date.to_string(),
        )
        .map_err(|error| {
            DeviceSyncError::resource(format!("sprite date {}", request.date), error)
        })?;
        sprites.date = Some(result.sha256);

        Ok(SyncResult { plans, sprites })
    }

    fn describe_error(&self, error: &Self::Error) -> SyncErrorReport {
        error.report()
    }
}
