use slint::{ComponentHandle, Weak};
use std::time::Duration;

use crate::chip_detection::detect_all_chips;
use crate::config;
use crate::csv_handler::CsvHandler;
use crate::serial::manager::SerialPortRegistry;
use crate::serial::modbus::{ModbusFrame, RegisterType};
use crate::{AppState, AppWindow};

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

    // IO芯片点击事件
    {
        let ui_weak = ui.as_weak();
        ui.global::<AppState>()
            .on_io_chip_click(move |chip_type, level, address| {
                handle_io_chip_click(ui_weak.clone(), chip_type.to_string(), level, address);
            });
    }

    // 读取文件按钮点击事件
    {
        let ui_weak = ui.as_weak();
        ui.global::<AppState>().on_read_file_clicked(move || {
            handle_read_file_click(ui_weak.clone());
        });
    }

    // 读取器件按钮点击事件
    {
        let ui_weak = ui.as_weak();
        ui.global::<AppState>().on_read_device_clicked(move || {
            handle_read_device_click(ui_weak.clone());
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

                    log::info!(
                        "芯片检测完成: 芯片1={:?}, 芯片2={:?}",
                        chip1_type,
                        chip2_type
                    );

                    // 启动IO状态轮询
                    start_io_polling(ui_weak.clone(), port.clone()).await;
                }
            }
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

async fn update_ui_status(
    ui_weak: &Weak<AppWindow>,
    status: &str,
    label: &str,
    is_connected: bool,
    show_chip_info: bool,
) {
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
    })
    .unwrap();
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
    })
    .unwrap();
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
    })
    .unwrap();
}

// 处理IO芯片点击事件
fn handle_io_chip_click(ui_weak: Weak<AppWindow>, chip_type: String, level: i32, address: i32) {
    // 先从UI中获取端口信息
    let port = if let Some(ui) = ui_weak.upgrade() {
        ui.global::<AppState>().get_port_value().to_string()
    } else {
        return;
    };

    config::get_runtime().spawn(async move {
        let registry = SerialPortRegistry::get_global().await;

        if let Some(port_manager) = registry.get_port(&port).await {
            let value = if level == 1 { 1u16 } else { 0u16 };
            let slave_address = if address >= 0x4000 && address < 0x8000 {
                1u8
            } else {
                2u8
            };

            match write_single_register(port_manager, slave_address, address as u16, value).await {
                Ok(()) => {
                    log::info!(
                        "IO设置成功: 芯片={}, 地址=0x{:04X}, 值={}",
                        chip_type,
                        address,
                        level
                    );
                }
                Err(e) => {
                    log::error!(
                        "IO设置失败: 芯片={}, 地址=0x{:04X}, 错误={}",
                        chip_type,
                        address,
                        e
                    );
                }
            }
        }
    });
}

// 启动IO状态轮询
async fn start_io_polling(ui_weak: Weak<AppWindow>, port: String) {
    let ui_weak_clone = ui_weak.clone();

    config::get_runtime().spawn(async move {
        poll_io_status(ui_weak_clone, port).await;
    });
}

// 轮询IO状态
async fn poll_io_status(ui_weak: Weak<AppWindow>, port: String) {
    let registry = SerialPortRegistry::get_global().await;
    log::info!("开始轮询IO状态: {}", port);

    loop {
        if let Some(port_manager) = registry.get_port(&port).await {
            // 逐个读取芯片一的IO状态
            let mut chip1_values = Vec::new();

            // 读取芯片一 IO1 (0x4001)
            match read_single_register(port_manager.clone(), 1, 0x4001).await {
                Ok(value) => chip1_values.push((value & 1) as i32),
                Err(e) => {
                    log::error!("读取芯片一IO1失败: {}", e);
                    chip1_values.push(0);
                }
            }

            // 读取芯片一 IO2 (0x4002)
            match read_single_register(port_manager.clone(), 1, 0x4002).await {
                Ok(value) => chip1_values.push((value & 1) as i32),
                Err(e) => {
                    log::error!("读取芯片一IO2失败: {}", e);
                    chip1_values.push(0);
                }
            }

            // 读取芯片一 IO3 (0x4003)
            match read_single_register(port_manager.clone(), 1, 0x4003).await {
                Ok(value) => chip1_values.push((value & 1) as i32),
                Err(e) => {
                    log::error!("读取芯片一IO3失败: {}", e);
                    chip1_values.push(0);
                }
            }

            // 逐个读取芯片二的IO状态
            let mut chip2_values = Vec::new();

            // 读取芯片二 IO1 (0xC001)
            match read_single_register(port_manager.clone(), 1, 0xC001).await {
                Ok(value) => chip2_values.push((value & 1) as i32),
                Err(e) => {
                    log::error!("读取芯片二IO1失败: {}", e);
                    chip2_values.push(0);
                }
            }

            // 读取芯片二 IO2 (0xC002)
            match read_single_register(port_manager.clone(), 1, 0xC002).await {
                Ok(value) => chip2_values.push((value & 1) as i32),
                Err(e) => {
                    log::error!("读取芯片二IO2失败: {}", e);
                    chip2_values.push(0);
                }
            }

            // 读取芯片二 IO3 (0xC003)
            match read_single_register(port_manager.clone(), 1, 0xC003).await {
                Ok(value) => chip2_values.push((value & 1) as i32),
                Err(e) => {
                    log::error!("读取芯片二IO3失败: {}", e);
                    chip2_values.push(0);
                }
            }

            // 更新UI状态
            update_io_status(&ui_weak, Ok(chip1_values), Ok(chip2_values)).await;

            tokio::time::sleep(Duration::from_millis(50000)).await;
        } else {
            log::warn!("端口 {} 不可用，停止轮询", port);
            break; // 如果端口不可用，退出循环
        }
    }
}

// 通用读单个寄存器方法
async fn read_single_register(
    port_manager: std::sync::Arc<crate::serial::base::SerialPortManager>,
    slave_address: u8,
    register_address: u16,
) -> Result<u16, String> {
    let command = match ModbusFrame::new_read_request(
        slave_address,
        RegisterType::HoldingRegister,
        register_address,
        1, // 读取1个寄存器
    ) {
        Ok(cmd) => cmd,
        Err(e) => return Err(format!("创建读命令失败: {}", e)),
    };

    match port_manager
        .send_modbus_command(&command.to_bytes(), 1000)
        .await
    {
        Ok(response) => {
            match ModbusFrame::from_bytes(&response) {
                Ok(frame) => {
                    let data = frame.get_data();
                    if data.len() >= 3 {
                        // 数据格式: [字节数, 高字节, 低字节]
                        let word = ((data[1] as u16) << 8) | (data[2] as u16);
                        Ok(word)
                    } else {
                        Err("响应数据长度不足".to_string())
                    }
                }
                Err(e) => Err(format!("解析响应失败: {}", e)),
            }
        }
        Err(e) => Err(format!("发送命令失败: {}", e)),
    }
}

// 通用写单个寄存器方法
async fn write_single_register(
    port_manager: std::sync::Arc<crate::serial::base::SerialPortManager>,
    slave_address: u8,
    register_address: u16,
    value: u16,
) -> Result<(), String> {
    let mut data = Vec::new();
    data.push((register_address >> 8) as u8); // 地址高字节
    data.push(register_address as u8); // 地址低字节
    data.push((value >> 8) as u8); // 值高字节
    data.push(value as u8); // 值低字节

    let command = ModbusFrame::new(slave_address, 0x06, data); // 功能码 0x06 写单个保持寄存器

    match port_manager
        .send_modbus_command(&command.to_bytes(), 1000)
        .await
    {
        Ok(response) => match ModbusFrame::from_bytes(&response) {
            Ok(_frame) => Ok(()),
            Err(e) => Err(format!("解析写响应失败: {}", e)),
        },
        Err(e) => Err(format!("发送写命令失败: {}", e)),
    }
}

// 更新IO状态到UI
async fn update_io_status(
    ui_weak: &Weak<AppWindow>,
    chip1_result: Result<Vec<i32>, String>,
    chip2_result: Result<Vec<i32>, String>,
) {
    let ui_weak_clone = ui_weak.clone();

    slint::invoke_from_event_loop(move || {
        if let Some(ui) = ui_weak_clone.upgrade() {
            match chip1_result {
                Ok(values) => {
                    let slint_values: slint::ModelRc<i32> =
                        slint::ModelRc::new(slint::VecModel::from(values));
                    ui.global::<AppState>().set_io_status1(slint_values);
                }
                Err(e) => {
                    log::error!("读取芯片一IO状态失败: {}", e);
                }
            }

            match chip2_result {
                Ok(values) => {
                    let slint_values: slint::ModelRc<i32> =
                        slint::ModelRc::new(slint::VecModel::from(values));
                    ui.global::<AppState>().set_io_status2(slint_values);
                }
                Err(e) => {
                    log::error!("读取芯片二IO状态失败: {}", e);
                }
            }
        }
    })
    .unwrap();
}

// 处理读取文件按钮点击事件
fn handle_read_file_click(ui_weak: Weak<AppWindow>) {
    let ui_weak_clone = ui_weak.clone();

    // 在后台线程中执行文件操作
    config::get_runtime().spawn(async move {
        // 执行CSV文件读取操作
        match CsvHandler::read_csv_file().await {
            Ok(table_content) => {
                log::info!("CSV文件读取成功，共解析 {} 字符", table_content.len());

                // 更新UI状态
                update_file_ui_success(&ui_weak_clone, table_content).await;
            }
            Err(e) => {
                log::error!("CSV文件读取失败: {}", e);

                // 更新UI错误状态
                update_file_ui_error(&ui_weak_clone, e.to_string()).await;
            }
        }
    });
}

// 更新文件操作UI状态 - 成功
async fn update_file_ui_success(ui_weak: &Weak<AppWindow>, content: String) {
    let ui_weak_clone = ui_weak.clone();
    let content_clone = content.clone();

    // 获取表格数据
    let table_data = match CsvHandler::get_slint_table_data().await {
        Ok(data) => data,
        Err(e) => {
            log::error!("获取表格数据失败: {}", e);
            vec![]
        }
    };

    slint::invoke_from_event_loop(move || {
        if let Some(ui) = ui_weak_clone.upgrade() {
            // 更新文件状态
            ui.global::<AppState>()
                .set_file_status("文件读取成功".into());
            ui.global::<AppState>()
                .set_file_status_color(slint::Brush::from(slint::Color::from_rgb_u8(40, 167, 69))); // 绿色

            // 更新文件内容显示（保留作为备用）
            ui.global::<AppState>()
                .set_file_content(content_clone.into());

            // 更新表格数据
            let table_model = slint::VecModel::from(
                table_data
                    .into_iter()
                    .map(|row| {
                        let row_model = slint::VecModel::from(
                            row.into_iter()
                                .map(|cell| slint::StandardListViewItem::from(cell))
                                .collect::<Vec<_>>(),
                        );
                        slint::ModelRc::new(row_model)
                    })
                    .collect::<Vec<_>>(),
            );
            ui.global::<AppState>()
                .set_csv_table_data(slint::ModelRc::new(table_model));

            log::info!("UI状态和表格数据更新完成");
        }
    })
    .unwrap();
}

// 更新文件操作UI状态 - 错误
async fn update_file_ui_error(ui_weak: &Weak<AppWindow>, error_msg: String) {
    let ui_weak_clone = ui_weak.clone();

    slint::invoke_from_event_loop(move || {
        if let Some(ui) = ui_weak_clone.upgrade() {
            // 更新文件状态为错误
            ui.global::<AppState>()
                .set_file_status(format!("读取失败: {}", error_msg).into());
            ui.global::<AppState>()
                .set_file_status_color(slint::Brush::from(slint::Color::from_rgb_u8(220, 53, 69))); // 红色

            // 清空文件内容
            ui.global::<AppState>().set_file_content("".into());

            // 清空表格数据
            let empty_table = slint::VecModel::from(vec![]);
            ui.global::<AppState>()
                .set_csv_table_data(slint::ModelRc::new(empty_table));
        }
    })
    .unwrap();
}

// 处理读取器件按钮点击事件
fn handle_read_device_click(ui_weak: Weak<AppWindow>) {
    // 提前获取串口路径，避免在异步任务中访问UI
    let port_path = if let Some(ui) = ui_weak.upgrade() {
        ui.global::<AppState>().get_port_value().to_string()
    } else {
        log::error!("UI界面已关闭");
        return;
    };

    let ui_weak_clone = ui_weak.clone();

    // 在后台线程中执行器件读取操作
    config::get_runtime().spawn(async move {
        // 执行器件读取操作
        match read_device_registers(&ui_weak_clone, &port_path).await {
            Ok(_) => {
                log::info!("器件读取完成");
                // 更新UI状态为成功
                update_device_read_ui_success(&ui_weak_clone).await;
            }
            Err(e) => {
                log::error!("器件读取失败: {}", e);
                // 更新UI错误状态
                update_device_read_ui_error(&ui_weak_clone, e.to_string()).await;
            }
        }
    });
}

// 执行器件寄存器读取操作
async fn read_device_registers(ui_weak: &Weak<AppWindow>, port_path: &str) -> anyhow::Result<()> {
    use crate::csv_handler::CsvHandler;
    use crate::serial::manager::SerialPortRegistry;

    // 获取串口注册表
    let registry = SerialPortRegistry::get_global().await;
    let records = CsvHandler::get_all_records().await?;

    // 获取当前连接的串口
    let port_manager = if let Some(manager) = registry.get_port(port_path).await {
        if manager.is_open() {
            manager
        } else {
            return Err(anyhow::anyhow!("串口 {} 未连接", port_path));
        }
    } else {
        return Err(anyhow::anyhow!("串口 {} 不存在", port_path));
    };

    let total_pages = records.len();
    let mut processed = 0;

    // 获取该页的所有寄存器记录

    for record in records {
        let page_addr = record.page_addr.clone();

        update_read_progress_status(ui_weak, processed, total_pages, &page_addr).await;
        // 只读取标记为可读的寄存器
        if record.r_w.to_uppercase().contains('R') {
            // 解析页地址（假设是十六进制格式）
            let page_addr_u16 = if page_addr.starts_with("0x") || page_addr.starts_with("0X") {
                u16::from_str_radix(&page_addr[2..], 16)
                    .map_err(|_| anyhow::anyhow!("无效的页地址格式: {}", page_addr))?
            } else {
                page_addr
                    .parse::<u16>()
                    .map_err(|_| anyhow::anyhow!("无效的页地址: {}", page_addr))?
            };

            match read_single_register(port_manager.clone(), 1, page_addr_u16).await {
                Ok(value) => {
                    // 更新寄存器的w_value
                    let hex_value = format!("0x{:02X}", value as u8);

                    log::info!(
                        "读取寄存器成功: {}:{} = {}",
                        page_addr,
                        record.register,
                        hex_value
                    );
                    if let Err(e) =
                        CsvHandler::update_w_value(&page_addr, &record.register, Some(hex_value))
                            .await
                    {
                        log::warn!("更新寄存器值失败: {}", e);
                    }
                }
                Err(e) => {
                    log::warn!("读取寄存器失败 {}:{} - {}", page_addr, record.register, e);
                }
            }

            // 添加小延时避免过于频繁的通信
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            processed += 1;
        }
    }

    // 完成后更新表格数据
    update_table_data_after_read(ui_weak).await?;

    Ok(())
}

// 更新读取进度状态
async fn update_read_progress_status(
    ui_weak: &Weak<AppWindow>,
    processed: usize,
    total: usize,
    current_page: &str,
) {
    let ui_weak_clone = ui_weak.clone();
    let status_text = format!(
        "正在读取器件数据... ({}/{}): {}",
        processed + 1,
        total,
        current_page
    );

    slint::invoke_from_event_loop(move || {
        if let Some(ui) = ui_weak_clone.upgrade() {
            ui.global::<AppState>().set_file_status(status_text.into());
            ui.global::<AppState>()
                .set_file_status_color(slint::Brush::from(slint::Color::from_rgb_u8(23, 162, 184))); // 蓝色
        }
    })
    .unwrap();
}

// 读取完成后更新表格数据
async fn update_table_data_after_read(ui_weak: &Weak<AppWindow>) -> anyhow::Result<()> {
    // 获取更新后的表格数据
    let table_data = CsvHandler::get_slint_table_data().await?;
    let ui_weak_clone = ui_weak.clone();

    slint::invoke_from_event_loop(move || {
        if let Some(ui) = ui_weak_clone.upgrade() {
            // 更新表格数据
            let table_model = slint::VecModel::from(
                table_data
                    .into_iter()
                    .map(|row| {
                        let row_model = slint::VecModel::from(
                            row.into_iter()
                                .map(|cell| slint::StandardListViewItem::from(cell))
                                .collect::<Vec<_>>(),
                        );
                        slint::ModelRc::new(row_model)
                    })
                    .collect::<Vec<_>>(),
            );
            ui.global::<AppState>()
                .set_csv_table_data(slint::ModelRc::new(table_model));
        }
    })
    .unwrap();

    Ok(())
}

// 更新器件读取UI状态 - 成功
async fn update_device_read_ui_success(ui_weak: &Weak<AppWindow>) {
    let ui_weak_clone = ui_weak.clone();

    slint::invoke_from_event_loop(move || {
        if let Some(ui) = ui_weak_clone.upgrade() {
            ui.global::<AppState>()
                .set_file_status("器件读取完成".into());
            ui.global::<AppState>()
                .set_file_status_color(slint::Brush::from(slint::Color::from_rgb_u8(40, 167, 69))); // 绿色
        }
    })
    .unwrap();
}

// 更新器件读取UI状态 - 错误
async fn update_device_read_ui_error(ui_weak: &Weak<AppWindow>, error_msg: String) {
    let ui_weak_clone = ui_weak.clone();

    slint::invoke_from_event_loop(move || {
        if let Some(ui) = ui_weak_clone.upgrade() {
            ui.global::<AppState>()
                .set_file_status(format!("器件读取失败: {}", error_msg).into());
            ui.global::<AppState>()
                .set_file_status_color(slint::Brush::from(slint::Color::from_rgb_u8(220, 53, 69))); // 红色
        }
    })
    .unwrap();
}
