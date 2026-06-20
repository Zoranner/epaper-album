use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyncErrorReport {
    pub code: String,
    pub category: String,
    pub stage: Option<String>,
    pub message: String,
    pub detail: String,
}

impl SyncErrorReport {
    pub fn new(
        code: impl Into<String>,
        category: impl Into<String>,
        stage: Option<String>,
        message: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            category: category.into(),
            stage,
            message: message.into(),
            detail: detail.into(),
        }
    }

    pub fn from_display(error: &impl fmt::Display) -> Self {
        Self::new(
            "sync.error",
            "sync",
            None,
            "CANNOT UPDATE SERVER DATA",
            error.to_string(),
        )
    }
}
