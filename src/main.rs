fn main() {
    init_runtime();
    run_device();
}

#[cfg(target_os = "espidf")]
fn run_device() {
    let wake = epaper_album::power::espidf::wake_probe();
    log::info!(target: "epaper_album", "wake: {}", wake.label());

    if epaper_album::power::espidf::self_test_key_long_pressed() {
        log::info!(target: "epaper_album", "self-test key: long-pressed");
        let report = epaper_album::hardware_selftest::run_espidf_hardware_self_test(wake);
        epaper_album::hardware_selftest::print_hardware_self_test_report(&report);
        return;
    }

    let trigger = match wake {
        epaper_album::power::espidf::WakeProbe::Timer => {
            epaper_album::app::RunTrigger::Wake(epaper_album::state::WakeReason::Timer)
        }
        epaper_album::power::espidf::WakeProbe::Button => {
            epaper_album::app::RunTrigger::Wake(epaper_album::state::WakeReason::Button)
        }
        epaper_album::power::espidf::WakeProbe::Unknown => epaper_album::app::RunTrigger::Startup,
        epaper_album::power::espidf::WakeProbe::Ulp
        | epaper_album::power::espidf::WakeProbe::Other(_) => {
            epaper_album::app::RunTrigger::Wake(epaper_album::state::WakeReason::External)
        }
    };
    let report = epaper_album::device_espidf::run_espidf_device_cycle(trigger);
    log::info!(
        target: "epaper_album",
        "device outcome: {}",
        report.outcome.label()
    );

    if let Some(cycle) = &report.cycle {
        log::info!(target: "epaper_album", "cycle outcome: {:?}", cycle.outcome);
        log::info!(target: "epaper_album", "sync attempted: {}", cycle.sync_attempted);
        log::info!(target: "epaper_album", "sync succeeded: {}", cycle.sync_succeeded);
        if let Some(error) = &cycle.persistent_state.last_sync_error {
            log::warn!(target: "epaper_album", "sync error: {error}");
        }
        log::info!(target: "epaper_album", "refresh attempted: {}", cycle.refresh_attempted);
        log::info!(target: "epaper_album", "refresh succeeded: {}", cycle.refresh_succeeded);
    }

    if let Some(sleep_plan) = report.sleep_plan {
        log::info!(
            target: "epaper_album",
            "next wake: {:?}, sleep seconds: {:?}",
            sleep_plan.next_wakeup_epoch_seconds,
            sleep_plan.deep_sleep_seconds
        );

        if let Some(seconds) = sleep_plan.deep_sleep_seconds.filter(|seconds| *seconds > 0) {
            if option_env!("EPAPER_ALBUM_ENTER_DEEP_SLEEP") == Some("1") {
                log::info!(target: "epaper_album", "sleep: entering deep sleep for {seconds}s");
                if let Err(error) =
                    epaper_album::power::espidf::enter_timer_deep_sleep(u64::from(seconds))
                {
                    log::warn!(target: "epaper_album", "sleep: enter-error: {error:?}");
                }
            } else {
                log::info!(
                    target: "epaper_album",
                    "sleep: planned {seconds}s, deep sleep disabled in this build"
                );
            }
        }
    }
}

#[cfg(not(target_os = "espidf"))]
fn run_device() {
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
