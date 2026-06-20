pub mod cycle;

use crate::state::WakeReason;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunTrigger {
    Startup,
    Wake(WakeReason),
}

impl RunTrigger {
    pub const fn wake_reason(self) -> WakeReason {
        match self {
            Self::Startup => WakeReason::Startup,
            Self::Wake(reason) => reason,
        }
    }
}
