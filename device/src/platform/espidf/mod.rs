use crate::device_runtime::{DeviceCycleOutcome, DeviceCycleResult};
use crate::pmic::espidf::PmicSleepProbe;
use crate::power::{DeepSleepWakePolicy, NextRunPlan};

mod diagnostics;
mod error_page;
mod runner;
mod schedule;
mod storage;
mod sync;

pub use runner::run_espidf_device_cycle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspDeviceRunReport {
    pub outcome: EspDeviceRunOutcome,
    pub cycle: Option<DeviceCycleResult>,
    pub next_run_plan: Option<NextRunPlan>,
    pub sleep_wake_policy: Option<DeepSleepWakePolicy>,
    pub sleep_probe: Option<PmicSleepProbe>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EspDeviceRunOutcome {
    Completed(DeviceCycleOutcome),
    SelfTest,
    PeripheralInitError,
    StorageMountError,
    EpdInitError,
    StateWriteError,
}

impl EspDeviceRunOutcome {
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Completed(_) => "completed",
            Self::SelfTest => "self-test",
            Self::PeripheralInitError => "peripheral-init-error",
            Self::StorageMountError => "storage-mount-error",
            Self::EpdInitError => "epd-init-error",
            Self::StateWriteError => "state-write-error",
        }
    }
}
