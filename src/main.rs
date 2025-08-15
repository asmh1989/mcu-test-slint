#![allow(dead_code)]
// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;

mod config;
mod serial;
mod serial_impl;
mod chip_detection;
mod ui_handlers;

slint::include_modules!();

fn main() -> Result<(), Box<dyn Error>> {
    config::init_config();
    log::info!("init_config");

    let ui = AppWindow::new()?;
    
    // 初始状态设置
    ui.global::<AppState>().set_mcu_label("连接".into());
    ui.global::<AppState>().set_is_connected(false);
    ui.global::<AppState>().set_connect_status("未连接".into());
    ui.global::<AppState>().set_show_chip_info(false);
    
    // 设置UI事件处理器
    ui_handlers::setup_ui_handlers(&ui);

    ui.run()?;

    Ok(())
}
