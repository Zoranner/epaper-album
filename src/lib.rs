pub mod app;
pub mod app_storage;
pub mod bmp;
pub mod cache;
pub mod cloud;
pub mod config;
pub mod device_runtime;
pub mod display;
pub mod epd;
pub mod error;
#[cfg(target_os = "espidf")]
pub mod hardware_selftest;
pub mod model;
pub mod pmic;
pub mod power;
pub mod render;
pub mod resource_sync;
pub mod schedule;
pub mod selftest;
pub mod state;
pub mod storage;
pub mod wifi;

pub use error::{AlbumError, AlbumResult};
