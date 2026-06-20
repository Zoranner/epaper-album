use crate::device_runtime::{DeviceDisplay, ErrorRefreshRequest};

pub fn refresh_storage_error_page<D>(display: &mut D, now_epoch_seconds: u64)
where
    D: DeviceDisplay,
{
    let result = display.refresh_error_page(ErrorRefreshRequest {
        title: "STORAGE ERROR".to_string(),
        message: "TF CARD IS NOT AVAILABLE".to_string(),
        hint: "CHECK TF CARD AND SLOT".to_string(),
        detail: "MOUNT /SDCARD FAILED".to_string(),
        now_epoch_seconds,
    });

    if let Err(error) = result {
        log::warn!(target: "epaper_album", "storage error page: {error}");
    }
}
