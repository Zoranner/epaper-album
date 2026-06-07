fn main() {
    init_runtime();
    run_self_test();
}

#[cfg(target_os = "espidf")]
fn run_self_test() {
    let wake = epaper_album::power::espidf::wake_probe();
    log::info!(target: "epaper_album", "wake: {}", wake.label());

    let report = epaper_album::hardware_selftest::run_espidf_hardware_self_test(wake);
    epaper_album::hardware_selftest::print_hardware_self_test_report(&report);
    sleep_until_timer_wakeup();
}

#[cfg(not(target_os = "espidf"))]
fn run_self_test() {
    use epaper_album::config::CONFIG_PATH;
    use epaper_album::selftest::{print_self_test_report, run_self_test as run_host_self_test};
    let report = run_host_self_test(CONFIG_PATH);
    print_self_test_report(&report);
}

#[cfg(target_os = "espidf")]
fn init_runtime() {
    esp_idf_sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
}

#[cfg(not(target_os = "espidf"))]
fn init_runtime() {}

#[cfg(target_os = "espidf")]
fn sleep_until_timer_wakeup() {
    let seconds = epaper_album::power::espidf::SELF_TEST_TIMER_WAKE_SECONDS;
    log::info!(target: "epaper_album", "sleep: timer {}s", seconds);

    if let Err(error) = epaper_album::power::espidf::enter_timer_deep_sleep(seconds) {
        log::error!(
            target: "epaper_album",
            "sleep: timer-error {:?}",
            error
        );
    }
}
