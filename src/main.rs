#![allow(dead_code)]
// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;

mod config;
mod serial;

slint::include_modules!();

fn main() -> Result<(), Box<dyn Error>> {
    let ui = AppWindow::new()?;

    // 连接回调
    ui.on_connect_clicked({
        let ui_handle = ui.as_weak();
        move || {
            let ui = ui_handle.unwrap();
            println!("连接按钮被点击");
            
            // 切换连接状态
            let current_status = ui.global::<AppState>().get_is_connected();
            ui.global::<AppState>().set_is_connected(!current_status);
            ui.global::<AppState>().set_connect_status(if !current_status { "已连接".into() } else { "未连接".into() });
            
            // TODO: 实现实际连接逻辑
        }
    });

    // 端口变化回调
    ui.on_port_changed({
        let ui_handle = ui.as_weak();
        move |port_text| {
            let _ui = ui_handle.unwrap();
            println!("端口变化为: {}", port_text);
            // TODO: 验证端口格式和可用性
        }
    });

    // 地址读取回调
    ui.on_read_address_clicked({
        let ui_handle = ui.as_weak();
        move || {
            let _ui = ui_handle.unwrap();
            println!("读取地址按钮被点击");
            // TODO: 实现读取地址逻辑
        }
    });

    // 地址写入回调
    ui.on_write_address_clicked({
        let ui_handle = ui.as_weak();
        move || {
            let _ui = ui_handle.unwrap();
            println!("写入地址按钮被点击");
            // TODO: 实现写入地址逻辑
        }
    });

    // IO1控制回调
    ui.on_io1_low_clicked({
        let ui_handle = ui.as_weak();
        move || {
            let ui = ui_handle.unwrap();
            println!("IO1置低按钮被点击");
            ui.global::<AppState>().set_io1_status(false);
            ui.global::<AppState>().set_io1_display("IO1: 低".into());
            // TODO: 实现IO1置低逻辑
        }
    });

    ui.on_io1_high_clicked({
        let ui_handle = ui.as_weak();
        move || {
            let ui = ui_handle.unwrap();
            println!("IO1置高按钮被点击");
            ui.global::<AppState>().set_io1_status(true);
            ui.global::<AppState>().set_io1_display("IO1: 高".into());
            // TODO: 实现IO1置高逻辑
        }
    });

    // IO2控制回调
    ui.on_io2_low_clicked({
        let ui_handle = ui.as_weak();
        move || {
            let ui = ui_handle.unwrap();
            println!("IO2置低按钮被点击");
            ui.global::<AppState>().set_io2_status(false);
            ui.global::<AppState>().set_io2_display("IO2: 低".into());
            // TODO: 实现IO2置低逻辑
        }
    });

    ui.on_io2_high_clicked({
        let ui_handle = ui.as_weak();
        move || {
            let ui = ui_handle.unwrap();
            println!("IO2置高按钮被点击");
            ui.global::<AppState>().set_io2_status(true);
            ui.global::<AppState>().set_io2_display("IO2: 高".into());
            // TODO: 实现IO2置高逻辑
        }
    });

    // 文件操作回调
    ui.on_read_file_clicked({
        let ui_handle = ui.as_weak();
        move || {
            let ui = ui_handle.unwrap();
            println!("读取文件按钮被点击");
            
            // 模拟文件读取操作
            // 成功情况
            ui.global::<AppState>().set_file_status("文件读取成功: D:\\config\\mcu_config.txt".into());
            ui.global::<AppState>().set_file_status_color(slint::Color::from_rgb_u8(40, 167, 69).into()); // 绿色
            
            // 错误情况示例（注释掉）
            // ui.global::<AppState>().set_file_status("读取错误: 文件不存在或无法访问".into());
            // ui.global::<AppState>().set_file_status_color(slint::Color::from_rgb_u8(220, 53, 69).into()); // 红色
            
            // TODO: 实现真实的文件读取逻辑
        }
    });

    ui.on_read_device_clicked({
        let ui_handle = ui.as_weak();
        move || {
            let ui = ui_handle.unwrap();
            println!("读取器件按钮被点击");
            
            // 模拟设备读取操作
            ui.global::<AppState>().set_file_status("设备读取中...".into());
            ui.global::<AppState>().set_file_status_color(slint::Color::from_rgb_u8(255, 193, 7).into()); // 黄色
            
            // TODO: 实现读取器件逻辑
        }
    });

    ui.on_config_file_clicked({
        let ui_handle = ui.as_weak();
        move || {
            let ui = ui_handle.unwrap();
            println!("配置器件按钮被点击");
            
            // 模拟配置操作
            ui.global::<AppState>().set_file_status("配置已写入设备".into());
            ui.global::<AppState>().set_file_status_color(slint::Color::from_rgb_u8(23, 162, 184).into()); // 蓝色
            
            // TODO: 实现配置器件逻辑
        }
    });

    ui.run()?;

    Ok(())
}
