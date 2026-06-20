fn main() {
    init_runtime();
    run_device();
}

#[cfg(target_os = "espidf")]
fn run_device() {
    let wake = epaper_album::power::espidf::wake_probe();
    log::info!(target: "epaper_album", "wake: {}", wake.label());

    let self_test_requested = match epaper_album::button::espidf::self_test_requested() {
        Ok(requested) => requested,
        Err(error) => {
            log::warn!(target: "epaper_album", "self-test request: read error: {error:?}");
            false
        }
    };

    if self_test_requested {
        log::info!(target: "epaper_album", "self-test request: entered");
        let report = epaper_album::hardware_selftest::run_espidf_hardware_self_test(wake);
        epaper_album::hardware_selftest::print_hardware_self_test_report(&report);
        loop {
            if epaper_album::power::espidf::self_test_key_clicked() {
                match epaper_album::button::espidf::clear_self_test_request() {
                    Ok(()) => {
                        log::info!(target: "epaper_album", "self-test request: cleared");
                        epaper_album::power::espidf::restart_now();
                    }
                    Err(error) => {
                        log::warn!(
                            target: "epaper_album",
                            "self-test request: clear error: {error:?}"
                        );
                    }
                }
            }
            esp_idf_hal::delay::FreeRtos::delay_ms(100);
        }
    }

    let trigger = match wake {
        epaper_album::power::espidf::WakeProbe::Timer => {
            epaper_album::app::RunTrigger::Wake(epaper_album::state::WakeReason::Timer)
        }
        epaper_album::power::espidf::WakeProbe::Button => {
            epaper_album::app::RunTrigger::Wake(epaper_album::state::WakeReason::Button)
        }
        epaper_album::power::espidf::WakeProbe::External => {
            epaper_album::app::RunTrigger::Wake(epaper_album::state::WakeReason::External)
        }
        epaper_album::power::espidf::WakeProbe::Unknown => epaper_album::app::RunTrigger::Startup,
        epaper_album::power::espidf::WakeProbe::Ulp
        | epaper_album::power::espidf::WakeProbe::Other(_) => {
            epaper_album::app::RunTrigger::Wake(epaper_album::state::WakeReason::External)
        }
    };
    log::info!(target: "epaper_album", "trigger: {:?}", trigger);

    let report = epaper_album::device_espidf::run_espidf_device_cycle(trigger);
    log::info!(
        target: "epaper_album",
        "device outcome: {}",
        report.outcome.label()
    );

    if let Some(cycle) = &report.cycle {
        log::info!(target: "epaper_album", "cycle outcome: {:?}", cycle.outcome);
        log::info!(
            target: "epaper_album",
            "sync decision: action={:?} cause={:?}",
            cycle.sync_decision.action,
            cycle.sync_decision.cause
        );
        log::info!(target: "epaper_album", "sync attempted: {}", cycle.sync_attempted);
        log::info!(target: "epaper_album", "sync succeeded: {}", cycle.sync_succeeded);
        if let Some(error) = &cycle.sync_error {
            log::warn!(target: "epaper_album", "sync error: {error}");
        }
        if let Some(report) = &cycle.sync_error_report {
            log::warn!(
                target: "epaper_album",
                "sync error report: code={} category={} stage={} message={} detail={}",
                report.code,
                report.category,
                report.stage.as_deref().unwrap_or(""),
                report.message,
                report.detail
            );
        }
        log_display_decision(&cycle.display_decision);
        log::info!(target: "epaper_album", "refresh attempted: {}", cycle.refresh_attempted);
        log::info!(target: "epaper_album", "refresh succeeded: {}", cycle.refresh_succeeded);
    }

    if let Some(next_run_plan) = report.next_run_plan {
        log::info!(
            target: "epaper_album",
            "next run: {}, wait seconds: {}",
            next_run_plan.next_run_epoch_seconds,
            next_run_plan.wait_seconds
        );

        if next_run_plan.wait_seconds > 0 {
            let externally_powered = report
                .cycle
                .as_ref()
                .is_some_and(|cycle| cycle.battery.externally_powered());
            let display_available = report
                .cycle
                .as_ref()
                .is_some_and(|cycle| cycle.display_available);
            if externally_powered && display_available {
                log::info!(
                    target: "epaper_album",
                    "next run: external power detected; waiting until absolute restart time"
                );
                epaper_album::power::espidf::restart_at_with_poll(
                    next_run_plan.next_run_epoch_seconds,
                    || {
                        if !epaper_album::power::espidf::self_test_key_long_pressed() {
                            return false;
                        }
                        log::info!(target: "epaper_album", "self-test key: request");
                        if let Err(error) = epaper_album::button::espidf::request_self_test() {
                            log::warn!(
                                target: "epaper_album",
                                "self-test request: write error: {error:?}"
                            );
                            return false;
                        }
                        true
                    },
                );
            } else if externally_powered {
                log::info!(
                    target: "epaper_album",
                    "next run: external power detected; waiting without key listener"
                );
                epaper_album::power::espidf::restart_at_with_poll(
                    next_run_plan.next_run_epoch_seconds,
                    || false,
                );
            } else {
                log::info!(
                    target: "epaper_album",
                    "next run: battery power detected; entering deep sleep"
                );
                if let Err(error) = epaper_album::power::espidf::enter_deep_sleep_until(
                    next_run_plan.next_run_epoch_seconds,
                ) {
                    log::warn!(target: "epaper_album", "next run: deep sleep error: {error:?}");
                }
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
fn log_display_decision(decision: &epaper_album::device_runtime::DisplayDecision) {
    use epaper_album::device_runtime::{DisplayAction, DisplayTarget};

    match &decision.action {
        DisplayAction::Keep => {
            log::info!(
                target: "epaper_album",
                "display decision: action=keep cause={:?}",
                decision.cause
            );
        }
        DisplayAction::Refresh(DisplayTarget::Photo {
            date,
            image,
            caption,
        }) => {
            log::info!(
                target: "epaper_album",
                "display decision: action=refresh-photo cause={:?} date={} image={} caption={}",
                decision.cause,
                date,
                image,
                caption
            );
        }
        DisplayAction::Refresh(DisplayTarget::Page { date, title, .. }) => {
            log::info!(
                target: "epaper_album",
                "display decision: action=refresh-page cause={:?} date={} title={}",
                decision.cause,
                date,
                title
            );
        }
    }
}

#[cfg(target_os = "espidf")]
fn init_runtime() {
    esp_idf_sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
}

#[cfg(not(target_os = "espidf"))]
fn init_runtime() {}
