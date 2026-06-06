use epaper_album::config::CONFIG_PATH;
use epaper_album::selftest::{print_self_test_report, run_self_test};

fn main() {
    init_runtime();

    let report = run_self_test(CONFIG_PATH);
    print_self_test_report(&report);
}

#[cfg(target_os = "espidf")]
fn init_runtime() {
    esp_idf_sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
}

#[cfg(not(target_os = "espidf"))]
fn init_runtime() {}
