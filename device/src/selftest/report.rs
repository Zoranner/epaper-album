use crate::selftest::hardware::{HardwareSelfTestReport, PmicSelfTestProbe};

pub fn print_hardware_self_test_report(report: &HardwareSelfTestReport) {
    log::info!(target: "inkframe_device", "Inkframe self-test");
    log::info!(target: "inkframe_device", "wake: {}", report.wake.label());
    log::info!(target: "inkframe_device", "pmic: {}", report.pmic.label());
    if let PmicSelfTestProbe::Ready(summary) = &report.pmic {
        log::info!(
            target: "inkframe_device",
            "power: chip=0x{:02x} axp2101={} vbus={} battery-present={} battery={:?} percent={:?} low={} effective-low={} dc=0x{:02x} ldo=0x{:02x}",
            summary.chip_id,
            summary.is_axp2101,
            summary.vbus_good,
            summary.battery_connected,
            summary.charge_state,
            summary.percent,
            summary.low_battery,
            summary.effective_low_battery,
            summary.dc_onoff,
            summary.ldo_onoff0
        );
    }
    log::info!(target: "inkframe_device", "storage: {}", report.base.storage.label());
    log::info!(target: "inkframe_device", "config: {}", report.base.config.label());
    log::info!(target: "inkframe_device", "base url: {}", report.base_url);
    log::info!(
        target: "inkframe_device",
        "epd: {}",
        report.epd.label()
    );
    log::info!(
        target: "inkframe_device",
        "wifi: {}",
        report.wifi.label()
    );
    log::info!(target: "inkframe_device", "wifi ssid: {}", report.ssid);
    log::info!(target: "inkframe_device", "wifi ip: {}", report.ip);
    log::info!(
        target: "inkframe_device",
        "http: {}",
        report.http.label()
    );
    log::info!(
        target: "inkframe_device",
        "wake marker: {}",
        report.wake_marker.label()
    );
    log::info!(
        target: "inkframe_device",
        "render refresh count: {}",
        report.base.render.refresh_count
    );
    log::info!(
        target: "inkframe_device",
        "render sleep: {}",
        report.base.render.slept
    );
}
