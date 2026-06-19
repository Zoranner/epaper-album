use std::{path::PathBuf, sync::Arc};

use chrono::{DateTime, Utc};
use tokio::sync::{mpsc, Mutex};

use crate::db::Store;

#[derive(Debug, Clone)]
pub struct AppState {
    pub store: Store,
    pub secret_key: String,
    pub admin_username: String,
    pub admin_password: String,
    pub admin_session: Arc<Mutex<AdminSession>>,
    pub data_dir: PathBuf,
    pub enqueue_processing: bool,
}

#[derive(Debug, Clone)]
pub struct AdminSession {
    pub token: String,
    pub expires_at: DateTime<Utc>,
}

impl AdminSession {
    pub fn new(token: String, expires_at: DateTime<Utc>) -> Self {
        Self { token, expires_at }
    }
}

#[derive(Debug, Clone)]
pub(super) struct RuntimeState {
    pub app: AppState,
    pub queue: Option<ProcessingQueue>,
}

#[derive(Debug, Clone)]
pub(super) struct ProcessingQueue {
    pub sender: mpsc::UnboundedSender<String>,
    pub queued: Arc<Mutex<std::collections::HashSet<String>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Permission {
    User,
    Admin,
}
