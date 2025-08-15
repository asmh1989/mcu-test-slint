use slint::{Weak, ComponentHandle};

use crate::serial::manager::SerialPortRegistry;
use crate::chip_detection::detect_all_chips;
use crate::config;
use crate::{AppWindow, AppState};

pub fn setup_ui_handlers(ui: &AppWindow) {
    // 连接按钮点击事件
    {
        let ui_weak = ui.as_weak();
        ui.global::<AppState>().on_connect_clicked(move || {
            start_connect(ui_weak.clone());
        });
    }
    
    // 端口变更事件
    {
        ui.global::<AppState>().on_port_changed(move |new_port| {
            log::info!("端口变更为: {}", new_port);
        });
    }
}

fn start_connect(ui_weak: Weak<AppWindow>) {
    let ui_weak2 = ui_weak.clone();
    if let Some(ui) = ui_weak2.upgrade() {
        // 使用config::get_runtime()而不是创建新的runtime
        let port = ui.global::<AppState>().get_port_value().to_string();
        log::info!("开始连接... {}", port);

        config::get_runtime().spawn(async move {
        handle_connect_click(ui_weak2, port).await;
    });
    }

}

async fn handle_connect_click(ui_weak: Weak<AppWindow>, port: String) {

    let registry = SerialPortRegistry::get_global().await;

    if registry.get_port(&port).await.is_none() {
        // 尝试连接
        log::info!("尝试连接串口: {}", port);

        // 更新UI状态 - 连接中
        update_ui_status(&ui_weak, "连接中...", "连接中", false, false).await;
        
        // 使用SerialPortRegistry::get_global()
        let registry = SerialPortRegistry::get_global().await;
        match registry.add_port_with_defaults(&port).await {
            Ok(_) => {
                update_ui_status(&ui_weak, "连接中", "断开", true, false).await;

                registry.open_all().await;


                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                if !registry.is_connected(&port).await {
                    log::error!("串口连接失败: {}", port);
                    update_ui_status(&ui_weak, "连接失败", "连接", false, false).await;
                    registry.remove_port(&port).await;
                    return;
                }

                log::info!("串口连接成功: {}", port);
                // 更新UI状态 - 已连接
                update_ui_status(&ui_weak, "已连接", "断开", true, false).await;

                // 等待一段时间确保连接稳定
                
                // 开始芯片检测
                if let Some(port_manager) = registry.get_port(&port).await {
                    log::info!("开始检测芯片类型...");
                    
                    let (chip1_type, chip2_type) = detect_all_chips(port_manager).await;
                    
                    let chip1_str = chip1_type.to_string();
                    let chip2_str = chip2_type.to_string();
                    
                    // 更新芯片信息
                    update_chip_info(&ui_weak, &chip1_str, &chip2_str).await;
                    
                    log::info!("芯片检测完成: 芯片1={:?}, 芯片2={:?}", chip1_type, chip2_type);
                }
            },
            Err(e) => {
                log::error!("串口连接失败: {}", e);
                
                // 更新UI状态 - 连接失败
                update_ui_status(&ui_weak, "连接失败", "连接", false, false).await;
            }
        }
    } else {
        // 断开连接
        log::info!("断开串口连接: {}", port);
        registry.remove_port(&port).await;

        // 清空芯片信息并更新UI状态
        clear_chip_info(&ui_weak).await;
        
        log::info!("串口已断开");
    }
}

async fn update_ui_status(ui_weak: &Weak<AppWindow>, status: &str, label: &str, is_connected: bool, show_chip_info: bool) {
    let ui_weak_clone = ui_weak.clone();
    let status = status.to_string();
    let label = label.to_string();
    
    slint::invoke_from_event_loop(move || {
        if let Some(ui) = ui_weak_clone.upgrade() {
            ui.global::<AppState>().set_connect_status(status.into());
            ui.global::<AppState>().set_mcu_label(label.into());
            ui.global::<AppState>().set_is_connected(is_connected);
            ui.global::<AppState>().set_show_chip_info(show_chip_info);
        }
    }).unwrap();
}

async fn update_chip_info(ui_weak: &Weak<AppWindow>, chip1_str: &str, chip2_str: &str) {
    let ui_weak_clone = ui_weak.clone();
    let chip1_str = chip1_str.to_string();
    let chip2_str = chip2_str.to_string();
    
    slint::invoke_from_event_loop(move || {
        if let Some(ui) = ui_weak_clone.upgrade() {
            ui.global::<AppState>().set_chip1_type(chip1_str.into());
            ui.global::<AppState>().set_chip2_type(chip2_str.into());
            ui.global::<AppState>().set_show_chip_info(true);
        }
    }).unwrap();
}

async fn clear_chip_info(ui_weak: &Weak<AppWindow>) {
    let ui_weak_clone = ui_weak.clone();
    
    slint::invoke_from_event_loop(move || {
        if let Some(ui) = ui_weak_clone.upgrade() {
            ui.global::<AppState>().set_chip1_type("".into());
            ui.global::<AppState>().set_chip2_type("".into());
            ui.global::<AppState>().set_show_chip_info(false);
            ui.global::<AppState>().set_is_connected(false);
            ui.global::<AppState>().set_connect_status("未连接".into());
            ui.global::<AppState>().set_mcu_label("连接".into());
        }
    }).unwrap();
}
