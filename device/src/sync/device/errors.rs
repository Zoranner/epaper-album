use crate::device_runtime::SyncErrorReport;
use crate::resource_sync::ResourceSyncError;
use thiserror::Error;

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum DeviceSyncError {
    #[error("{stage}: {source}")]
    Resource {
        stage: String,
        #[source]
        source: ResourceSyncError,
    },
}

impl DeviceSyncError {
    pub(crate) fn resource(stage: impl Into<String>, source: ResourceSyncError) -> Self {
        Self::Resource {
            stage: stage.into(),
            source,
        }
    }

    pub fn code(&self) -> String {
        match self {
            Self::Resource { source, .. } => format!("resource.{}", source.code()),
        }
    }

    pub const fn category(&self) -> &'static str {
        match self {
            Self::Resource { .. } => "resource",
        }
    }

    pub fn stage(&self) -> &str {
        match self {
            Self::Resource { stage, .. } => stage,
        }
    }

    pub fn message(&self) -> String {
        format!("{} sync failed", self.stage())
    }

    pub fn detail(&self) -> Option<String> {
        match self {
            Self::Resource { source, .. } => Some(source.to_string()),
        }
    }

    pub fn report(&self) -> SyncErrorReport {
        SyncErrorReport::new(
            self.code(),
            self.category(),
            Some(self.stage().to_string()),
            self.message(),
            self.detail().unwrap_or_else(|| self.to_string()),
        )
    }
}
