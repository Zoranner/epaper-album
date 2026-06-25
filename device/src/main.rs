fn main() {
    init_runtime();
    run_device();
}

#[cfg(target_os = "espidf")]
fn run_device() {
    let wake = inkframe_device::power::espidf::wake_probe();
    log::info!(target: "inkframe_device", "wake: {}", wake.label());

    let self_test_requested = match inkframe_device::button::espidf::self_test_requested() {
        Ok(requested) => requested,
        Err(error) => {
            log::warn!(target: "inkframe_device", "self-test request: read error: {error:?}");
            false
        }
    };

    if self_test_requested {
        log::info!(target: "inkframe_device", "self-test request: entered");
        let report = inkframe_device::hardware_selftest::run_espidf_hardware_self_test(wake);
        inkframe_device::hardware_selftest::print_hardware_self_test_report(&report);
        loop {
            if inkframe_device::power::espidf::self_test_key_clicked() {
                log::info!(target: "inkframe_device", "self-test key: exit");
                inkframe_device::hardware_selftest::play_self_test_key_tone();
                match inkframe_device::button::espidf::clear_self_test_request() {
                    Ok(()) => {
                        log::info!(target: "inkframe_device", "self-test request: cleared");
                        inkframe_device::power::espidf::restart_now();
                    }
                    Err(error) => {
                        log::warn!(
                            target: "inkframe_device",
                            "self-test request: clear error: {error:?}"
                        );
                    }
                }
            }
            esp_idf_hal::delay::FreeRtos::delay_ms(100);
        }
    }

    let trigger = match wake {
        inkframe_device::power::espidf::WakeProbe::Timer => {
            inkframe_device::app::RunTrigger::Wake(inkframe_device::state::WakeReason::Timer)
        }
        inkframe_device::power::espidf::WakeProbe::Button => {
            inkframe_device::app::RunTrigger::Wake(inkframe_device::state::WakeReason::Button)
        }
        inkframe_device::power::espidf::WakeProbe::External => {
            inkframe_device::app::RunTrigger::Wake(inkframe_device::state::WakeReason::External)
        }
        inkframe_device::power::espidf::WakeProbe::Unknown => {
            inkframe_device::app::RunTrigger::Startup
        }
        inkframe_device::power::espidf::WakeProbe::Ulp
        | inkframe_device::power::espidf::WakeProbe::Other(_) => {
            inkframe_device::app::RunTrigger::Wake(inkframe_device::state::WakeReason::External)
        }
    };
    log::info!(target: "inkframe_device", "trigger: {:?}", trigger);

    let report = inkframe_device::device_espidf::run_espidf_device_cycle(trigger);
    log::info!(
        target: "inkframe_device",
        "device outcome: {}",
        report.outcome.label()
    );

    if let Some(cycle) = &report.cycle {
        log::info!(target: "inkframe_device", "cycle outcome: {:?}", cycle.outcome);
        log::info!(
            target: "inkframe_device",
            "sync decision: action={:?} cause={:?}",
            cycle.sync_decision.action,
            cycle.sync_decision.cause
        );
        log::info!(target: "inkframe_device", "sync attempted: {}", cycle.sync_attempted);
        log::info!(target: "inkframe_device", "sync succeeded: {}", cycle.sync_succeeded);
        if let Some(error) = &cycle.sync_error {
            log::warn!(target: "inkframe_device", "sync error: {error}");
        }
        if let Some(report) = &cycle.sync_error_report {
            log::warn!(
                target: "inkframe_device",
                "sync error report: code={} category={} stage={} message={} detail={}",
                report.code,
                report.category,
                report.stage.as_deref().unwrap_or(""),
                report.message,
                report.detail
            );
        }
        log_display_decision(&cycle.display_decision);
        log::info!(target: "inkframe_device", "refresh attempted: {}", cycle.refresh_attempted);
        log::info!(target: "inkframe_device", "refresh succeeded: {}", cycle.refresh_succeeded);
    }

    if let Some(next_run_plan) = report.next_run_plan {
        log::info!(
            target: "inkframe_device",
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
                    target: "inkframe_device",
                    "next run: external power detected; waiting until absolute restart time"
                );
                inkframe_device::power::espidf::restart_at_with_poll(
                    next_run_plan.next_run_epoch_seconds,
                    || {
                        if !inkframe_device::power::espidf::self_test_key_long_pressed() {
                            return false;
                        }
                        log::info!(target: "inkframe_device", "self-test key: request");
                        inkframe_device::hardware_selftest::play_self_test_key_tone();
                        if let Err(error) = inkframe_device::button::espidf::request_self_test() {
                            log::warn!(
                                target: "inkframe_device",
                                "self-test request: write error: {error:?}"
                            );
                            return false;
                        }
                        true
                    },
                );
            } else if externally_powered {
                log::info!(
                    target: "inkframe_device",
                    "next run: external power detected; waiting without key listener"
                );
                inkframe_device::power::espidf::restart_at_with_poll(
                    next_run_plan.next_run_epoch_seconds,
                    || false,
                );
            } else {
                log::info!(
                    target: "inkframe_device",
                    "next run: battery power detected; entering deep sleep"
                );
                let Some(wake_policy) = report.sleep_wake_policy else {
                    log::warn!(target: "inkframe_device", "next run: deep sleep skipped because wake policy is missing");
                    return;
                };
                log::info!(
                    target: "inkframe_device",
                    "next run: deep sleep wake policy: {}",
                    wake_policy.label()
                );
                if let Err(error) = inkframe_device::power::espidf::enter_deep_sleep_until(
                    next_run_plan.next_run_epoch_seconds,
                    wake_policy,
                ) {
                    log::warn!(target: "inkframe_device", "next run: deep sleep error: {error:?}");
                }
            }
        }
    }
}

#[cfg(not(target_os = "espidf"))]
fn run_device() {
    use inkframe_device::config::CONFIG_PATH;
    use inkframe_device::selftest::{print_self_test_report, run_self_test as run_host_self_test};
    let report = run_host_self_test(CONFIG_PATH);
    print_self_test_report(&report);
}

#[cfg(target_os = "espidf")]
fn log_display_decision(decision: &inkframe_device::device_runtime::DisplayDecision) {
    use inkframe_device::device_runtime::{DisplayAction, DisplayTarget};

    match &decision.action {
        DisplayAction::Keep => {
            log::info!(
                target: "inkframe_device",
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
                target: "inkframe_device",
                "display decision: action=refresh-photo cause={:?} date={} image={} caption={}",
                decision.cause,
                date,
                image,
                caption
            );
        }
        DisplayAction::Refresh(DisplayTarget::Page { date, title, .. }) => {
            log::info!(
                target: "inkframe_device",
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
