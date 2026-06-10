pub mod app;
pub mod config;
pub mod domain;
pub mod error;
pub mod graphics;
#[cfg(target_os = "espidf")]
pub mod hardware_selftest {
    pub use crate::selftest::hardware::*;
}
pub mod model;
pub mod platform;
pub mod power;
pub mod selftest;
pub mod storage;
pub mod sync;

pub mod app_storage {
    pub use crate::storage::app_files::*;
}

pub mod bmp {
    pub use crate::graphics::bmp::*;
}

pub mod cache {
    pub use crate::domain::cache::*;
}

pub mod cloud {
    pub use crate::sync::cloud::*;
}

pub mod device_display {
    pub use crate::graphics::device_display::*;
}

pub mod device_espidf {
    #[cfg(target_os = "espidf")]
    pub use crate::platform::espidf::*;
}

pub mod device_runtime {
    pub use crate::app::cycle::*;
}

pub mod device_sync {
    pub use crate::sync::device::*;
}

pub mod display {
    pub use crate::graphics::display::*;
}

pub mod epd {
    pub use crate::graphics::epd::*;
}

pub mod pmic {
    #[cfg(target_os = "espidf")]
    pub use crate::platform::pmic::*;
}

pub mod render {
    pub use crate::graphics::render::*;
}

pub mod resource_sync {
    pub use crate::sync::resource::*;
}

pub mod schedule {
    pub use crate::domain::schedule::*;
}

pub mod state {
    pub use crate::domain::state::*;
}

pub mod wifi {
    #[cfg(target_os = "espidf")]
    pub use crate::platform::wifi::*;
}

pub use error::{AlbumError, AlbumResult};
