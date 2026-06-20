mod display_decision;
mod error_page;
mod ports;
mod runner;
mod sync_decision;

#[cfg(test)]
mod runner_tests;
#[cfg(test)]
mod test_support;

pub use crate::app::RunTrigger;
pub use display_decision::{
    decide_display, DisplayAction, DisplayCause, DisplayDecision, DisplayTarget, RunContext,
};
pub use error_page::SyncErrorReport;
pub use ports::{
    DeviceCloudSync, DeviceDisplay, DisplayRefreshRequest, ErrorRefreshRequest, SpriteSet,
    SyncRequest, SyncResult,
};
pub use runner::{run_device_cycle, DeviceCycleInput, DeviceCycleOutcome, DeviceCycleResult};
pub use sync_decision::{decide_sync, SyncAction, SyncCause, SyncDecision};
