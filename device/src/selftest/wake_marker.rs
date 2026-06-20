use crate::power::espidf::WakeProbe;
use std::path::Path;

pub const WAKE_TEST_MARKER_PATH: &str = "/sdcard/wake-test.txt";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WakeMarkerProbe {
    Timer,
    Unknown,
    Missing,
    ReadError,
    WriteError,
}

impl WakeMarkerProbe {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Timer => "timer",
            Self::Unknown => "unknown",
            Self::Missing => "missing",
            Self::ReadError => "read-error",
            Self::WriteError => "write-error",
        }
    }
}

pub fn probe_wake_marker(path: &Path, wake: WakeProbe) -> WakeMarkerProbe {
    if matches!(wake, WakeProbe::Timer) && std::fs::write(path, wake.label()).is_err() {
        return WakeMarkerProbe::WriteError;
    }

    match std::fs::read_to_string(path) {
        Ok(content) if content.trim() == WakeProbe::Timer.label() => WakeMarkerProbe::Timer,
        Ok(_) => WakeMarkerProbe::Unknown,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => WakeMarkerProbe::Missing,
        Err(_) => WakeMarkerProbe::ReadError,
    }
}
