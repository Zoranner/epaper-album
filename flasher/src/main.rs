#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

slint::include_modules!();

use flasher::{discover_ports, flash_firmware, PortEntry};
use slint::{ModelRc, SharedString, VecModel};
use std::path::PathBuf;
use std::rc::Rc;
use std::thread;

fn main() {
    std::env::set_var("SLINT_BACKEND", "winit-software");

    let ui = App::new().unwrap();
    ui.set_ports(string_model(vec!["正在扫描...".to_string()]));
    ui.set_selected_port(SharedString::from(""));
    ui.set_log(SharedString::from("等待操作..."));
    ui.set_busy(false);
    refresh_ports(ui.as_weak());

    let ui_weak = ui.as_weak();
    ui.on_refresh_ports(move || {
        refresh_ports(ui_weak.clone());
    });

    let ui_weak = ui.as_weak();
    ui.on_choose_image(move || {
        if let Some(ui) = ui_weak.upgrade() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Merged firmware", &["bin"])
                .pick_file()
            {
                ui.set_image_path(path.to_string_lossy().to_string().into());
                ui.set_log(format!("已选择文件: {}", path.display()).into());
            }
        }
    });

    let ui_weak = ui.as_weak();
    ui.on_flash(move || {
        let ui_weak = ui_weak.clone();
        start_flash(ui_weak);
    });

    ui.run().unwrap();
}

fn refresh_ports(ui_weak: slint::Weak<App>) {
    if let Some(ui) = ui_weak.upgrade() {
        ui.set_busy(true);
        ui.set_log("正在扫描串口...".into());
    }

    thread::spawn(move || {
        let result = discover_ports();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_busy(false);
                match result {
                    Ok(list) => {
                        ui.set_ports(port_names_model(&list));
                        if let Some(first) = list.first() {
                            ui.set_selected_port(first.name.clone().into());
                        } else {
                            ui.set_selected_port(SharedString::from(""));
                        }
                        ui.set_log(port_scan_log(&list).into());
                    }
                    Err(err) => {
                        ui.set_ports(string_model(vec!["扫描失败".to_string()]));
                        ui.set_selected_port(SharedString::from(""));
                        ui.set_log(err.into());
                    }
                }
            }
        });
    });
}

fn port_names_model(ports: &[PortEntry]) -> ModelRc<SharedString> {
    if ports.is_empty() {
        string_model(vec!["未发现串口".to_string()])
    } else {
        string_model(ports.iter().map(|port| port.name.clone()).collect())
    }
}

fn string_model(items: Vec<String>) -> ModelRc<SharedString> {
    Rc::new(VecModel::from(
        items
            .into_iter()
            .map(SharedString::from)
            .collect::<Vec<_>>(),
    ))
    .into()
}

fn port_scan_log(ports: &[PortEntry]) -> String {
    if ports.is_empty() {
        return "未发现串口，请确认设备已连接并安装驱动。".to_string();
    }

    let mut lines = vec![format!("已刷新串口列表，共 {} 项", ports.len())];
    lines.extend(ports.iter().map(|port| port.display_name.clone()));
    lines.join("\n")
}

fn start_flash(ui_weak: slint::Weak<App>) {
    let Some(ui) = ui_weak.upgrade() else {
        return;
    };

    let port = ui.get_selected_port().to_string();
    let image = PathBuf::from(ui.get_image_path().to_string());
    ui.set_busy(true);
    ui.set_log(
        format!(
            "准备烧录...\n串口: {}\n固件: {}",
            if port.trim().is_empty() {
                "<未选择>"
            } else {
                port.as_str()
            },
            image.display()
        )
        .into(),
    );

    thread::spawn(move || {
        let result = flash_firmware(&port, &image);
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_busy(false);
                match result {
                    Ok(message) => ui.set_log(message.into()),
                    Err(err) => ui.set_log(err.to_string().into()),
                }
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn port_scan_log_reports_no_ports() {
        assert!(port_scan_log(&[]).contains("未发现串口"));
    }

    #[test]
    fn port_scan_log_includes_display_names() {
        let ports = vec![PortEntry {
            name: "COM3".to_string(),
            display_name: "COM3 - USB 303a:1001".to_string(),
        }];

        let log = port_scan_log(&ports);

        assert!(log.contains("共 1 项"));
        assert!(log.contains("COM3 - USB 303a:1001"));
    }
}
