use std::{collections::HashMap, sync::Arc};

use futures_util::future::join_all;
use tokio::sync::{Mutex, OnceCell, mpsc};
use tokio_util::sync::CancellationToken;

use crate::serial::base::SerialPortManager;

// 定义串口事件类型
#[derive(Debug, Clone)]
pub enum SerialPortEvent {
    /// 串口已添加到监听列表
    PortAddedToMonitoring { port: String },
    /// 串口已从系统中移除
    PortRemovedFromSystem { port: String },
    /// 串口已重新连接
    PortReconnected { port: String },
    /// 串口连接失败
    PortConnectionFailed { port: String, error: String },
    /// 串口状态更新
    PortStatusUpdate {
        port: String,
        is_connected: bool,
        is_available: bool,
    },
}

// 事件发送器类型
pub type SerialEventSender = mpsc::UnboundedSender<SerialPortEvent>;
// 事件接收器类型
pub type SerialEventReceiver = mpsc::UnboundedReceiver<SerialPortEvent>;

// 定义一个结构体来管理多个串口
pub struct SerialPortRegistry {
    // 使用 HashMap 存储 SerialPortManager 实例，键为串口路径
    ports: Mutex<HashMap<String, Arc<SerialPortManager>>>,
    // 注册表级别的取消令牌，用于通知所有管理的串口管理器及其任务退出
    registry_cancel_token: CancellationToken,
    task_ports: Mutex<Vec<String>>, // 新增
    // 新增默认参数
    default_baud_rate: u32,
    default_read_timeout_ms: u64,
    default_data_channel_buffer_size: usize,
    // 事件发送器列表 - 支持多个订阅者
    event_senders: Mutex<Vec<SerialEventSender>>,
}

static GLOBAL_REGISTRY: OnceCell<Arc<SerialPortRegistry>> = OnceCell::const_new();

impl SerialPortRegistry {
    // 构造函数
    // 返回 SerialPortRegistry 实例
    pub fn new() -> Arc<Self> {
        let registry = Arc::new(Self {
            ports: Mutex::new(HashMap::new()),
            registry_cancel_token: CancellationToken::new(),
            task_ports: Mutex::new(Vec::new()), // 新增
            default_baud_rate: 115200,
            default_read_timeout_ms: 200,
            default_data_channel_buffer_size: 8,
            event_senders: Mutex::new(Vec::new()),
        });

        // 在创建注册表时启动统一的监测任务
        registry.clone().start_monitoring_task();

        registry // 返回注册表实例
    }
    /// 新增port 到 taskPorts
    pub async fn add_task_port(&self, port: &str) {
        let mut task_ports = self.task_ports.lock().await;
        if !task_ports.contains(&port.to_string()) {
            task_ports.push(port.to_string());
            log::info!("新增监测端口 {}", port);

            // 触发事件
            self.emit_event(SerialPortEvent::PortAddedToMonitoring {
                port: port.to_string(),
            })
            .await;
        }
    }

    /// 创建事件订阅，返回接收器
    pub async fn subscribe_events(&self) -> SerialEventReceiver {
        let (sender, receiver) = mpsc::unbounded_channel();

        // 将发送器添加到列表中
        let mut senders = self.event_senders.lock().await;
        senders.push(sender);

        receiver
    }

    /// 发送事件到所有订阅者（非阻塞）
    async fn emit_event(&self, event: SerialPortEvent) {
        let mut senders = self.event_senders.lock().await;

        // 过滤掉已关闭的发送器，同时发送事件到活跃的发送器
        let mut active_senders = Vec::new();

        for sender in senders.drain(..) {
            // 尝试发送事件，如果失败说明接收器已关闭
            if sender.send(event.clone()).is_ok() {
                active_senders.push(sender);
            } else {
                log::debug!("移除已关闭的事件订阅者");
            }
        }

        // 更新活跃的发送器列表
        *senders = active_senders;
    }

    // 启动一个任务来监测所有注册的串口连接并尝试重连
    fn start_monitoring_task(self: Arc<Self>) {
        let cancel_token = self.registry_cancel_token.clone();
        let registry = Arc::clone(&self);
        tokio::spawn(async move {
            log::info!("注册表监测任务: 启动");
            loop {
                tokio::select! {
                    _ = cancel_token.cancelled() => {
                        log::info!("注册表监测任务: 收到取消信号，睡眠中退出");
                        break;
                    }
                    _ = tokio::time::sleep(tokio::time::Duration::from_millis(500)) => {}
                }

                // 1. 获取当前可用的端口列表
                let available_ports = match tokio_serial::available_ports() {
                    Ok(ports) => ports.into_iter().map(|p| p.port_name).collect::<Vec<_>>(),
                    Err(e) => {
                        log::error!("获取可用串口列表失败: {}", e);
                        continue;
                    }
                };

                // 2. 获取当前已打开过的ports
                let ports_list: Vec<String>;
                {
                    let ports_guard = registry.ports.lock().await;
                    ports_list = ports_guard.keys().cloned().collect();
                }

                // 3. 获取当前监测的task_ports
                let mut task_ports_guard = registry.task_ports.lock().await;
                let task_ports = task_ports_guard.clone();

                // 3.1 如果taskPorts不包含ports中的一个, 则把这个加入到taskPorts
                for port in &ports_list {
                    if !task_ports.contains(port) {
                        task_ports_guard.push(port.clone());
                        log::info!("监测任务: 新增监测端口 {}", port);

                        // 触发事件
                        registry
                            .emit_event(SerialPortEvent::PortAddedToMonitoring {
                                port: port.clone(),
                            })
                            .await;
                    }
                }

                // 3.2 如果availablePorts中没有, 而ports中有, 则remove_port
                for port in &ports_list {
                    if !available_ports.contains(port) {
                        log::info!("监测任务: 端口 {} 已不在系统中, 移除", port);

                        // 触发事件
                        registry
                            .emit_event(SerialPortEvent::PortRemovedFromSystem {
                                port: port.clone(),
                            })
                            .await;

                        registry.remove_port(port).await;
                    }
                }

                let mut reopen = false;

                // 3.3 如果availablePorts和TaskPorts中都有, 而ports中没有, 则add_port
                for port in &available_ports {
                    if task_ports.contains(port) && !ports_list.contains(port) {
                        log::info!(
                            "监测任务: 端口 {} 在系统和监测列表中, 但未注册, 自动添加",
                            port
                        );
                        // 这里需要指定默认参数, 你可以根据实际情况调整
                        match registry.add_scpi_port(port, 9600).await {
                            Ok(_) => {
                                // 触发重新连接事件
                                registry
                                    .emit_event(SerialPortEvent::PortReconnected {
                                        port: port.clone(),
                                    })
                                    .await;
                                reopen = true; // 标记需要重新打开
                            }
                            Err(e) => {
                                // 触发连接失败事件
                                registry
                                    .emit_event(SerialPortEvent::PortConnectionFailed {
                                        port: port.clone(),
                                        error: e.to_string(),
                                    })
                                    .await;
                            }
                        }
                    }
                }

                // 发送所有端口的状态更新事件
                for port in &available_ports {
                    if let Some(manager) = registry.get_port(port).await {
                        registry
                            .emit_event(SerialPortEvent::PortStatusUpdate {
                                port: port.clone(),
                                is_connected: manager.is_open(),
                                is_available: true,
                            })
                            .await;
                    }
                }

                if reopen {
                    registry.open_all().await;
                }
            }
            log::info!("注册表监测任务: 退出");
        });
    }

    // 新增：带默认参数的 add_port
    pub async fn add_port_with_defaults(&self, port_path: &str) -> anyhow::Result<()> {
        self.add_port(
            port_path,
            self.default_baud_rate,
            self.default_read_timeout_ms,
            self.default_data_channel_buffer_size,
        )
        .await
    }

    // 添加并启动一个新的串口管理器
    // data_channel_buffer_size: 内部数据通道的缓冲区大小
    pub async fn add_port(
        &self,
        port_path: &str,
        baud_rate: u32,
        read_timeout_ms: u64,
        data_channel_buffer_size: usize,
    ) -> anyhow::Result<()> {
        let mut ports = self.ports.lock().await;
        if ports.contains_key(port_path) {
            log::warn!("串口 {} 已存在于注册表中", port_path);
            // 如果需要，可以返回错误或更新现有端口
            return Err(anyhow::anyhow!("串口 {} 已存在", port_path));
        }

        // 为新的串口管理器创建一个子取消令牌
        let port_cancel_token = self.registry_cancel_token.child_token();

        // 创建一个新的 SerialPortManager 实例
        // SerialPortManager::new 内部会创建自己的通道并启动接收和数据处理任务
        let port_manager = SerialPortManager::new(
            port_path,
            baud_rate,
            read_timeout_ms,
            port_cancel_token,        // 传递子令牌
            data_channel_buffer_size, // 传递内部通道缓冲区大小
            true,                     // 启用自动接收任务 (传统模式)
        );

        // 存储管理器
        ports.insert(port_path.to_string(), Arc::clone(&port_manager));

        log::info!("串口 {} 已添加到注册表并启动内部任务", port_path);
        Ok(())
    }

    // SCPI通信专用的端口添加方法 - 禁用自动接收任务
    pub async fn add_scpi_port(&self, port_path: &str, baud_rate: u32) -> anyhow::Result<()> {
        let mut ports = self.ports.lock().await;
        if ports.contains_key(port_path) {
            log::warn!("SCPI串口 {} 已存在于注册表中", port_path);
            return Err(anyhow::anyhow!("串口 {} 已存在", port_path));
        }

        // 为新的串口管理器创建一个子取消令牌
        let port_cancel_token = self.registry_cancel_token.child_token();

        // 使用SCPI专用构造函数创建管理器
        let port_manager = SerialPortManager::new_for_scpi(port_path, baud_rate, port_cancel_token);

        // 存储管理器
        ports.insert(port_path.to_string(), Arc::clone(&port_manager));

        log::info!("SCPI串口 {} 已添加到注册表 (禁用自动接收任务)", port_path);
        Ok(())
    }

    // 打开所有注册的串口
    pub async fn open_all(&self) {
        let ports = self.ports.lock().await;
        // 使用 futures::future::join_all 并行打开所有串口
        let open_futures: Vec<_> = ports
            .values()
            .filter(|manager| !manager.is_open()) // 只打开未打开的端口
            .map(|manager| {
                let manager = Arc::clone(manager);
                tokio::spawn(async move {
                    if let Err(e) = manager.open().await {
                        log::error!("打开串口 {} 失败: {}", manager.get_port(), e);
                    }
                })
            })
            .collect();

        // 等待所有打开任务完成 (可选，取决于需求)
        join_all(open_futures).await;
    }

    // 关闭所有注册的串口并停止相关任务
    pub async fn close_all(&self) {
        // 触发注册表级别的取消令牌，通知所有子任务 (包括所有接收任务、数据处理任务和统一监测任务) 退出
        self.registry_cancel_token.cancel();
        log::info!("触发注册表取消令牌，通知所有任务退出...");

        // 给任务一些时间来响应取消信号并退出
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        // 显式关闭所有串口 (可选，因为丢弃 Arc 会最终关闭，但显式调用可以更新 is_connected 状态)
        // 注意：这里获取锁是为了遍历，close() 内部会再次尝试获取锁，但因为任务正在退出，冲突的可能性降低
        let ports = self.ports.lock().await;
        let close_futures: Vec<_> = ports
            .values()
            .map(|manager| {
                let manager = Arc::clone(manager);
                tokio::spawn(async move {
                    manager.close().await; // close() 内部会设置 is_connected 为 false 并触发其自身的子令牌 (已由 registry_cancel_token 触发)
                })
            })
            .collect();

        //等待所有关闭任务完成 (可选)
        join_all(close_futures).await;

        log::info!("所有注册串口已关闭");
    }

    /// 发送数据到指定串口
    pub async fn send_data(&self, port_path: &str, data: &[u8]) -> anyhow::Result<()> {
        if let Some(manager) = self.get_port(port_path).await {
            manager.send(data).await
        } else {
            Err(anyhow::anyhow!("串口 {} 未找到", port_path))
        }
    }

    /// 发送数据到所有连接的串口
    pub async fn send_data_to_all(&self, data: &[u8]) -> anyhow::Result<()> {
        let ports = self.ports.lock().await;
        let send_futures: Vec<_> = ports
            .values()
            .map(|manager| {
                let manager = Arc::clone(manager);
                let data = data.to_vec(); // Clone data for each task
                tokio::spawn(async move {
                    if let Err(e) = manager.send(&data).await {
                        log::error!("发送数据到串口 {} 失败: {}", manager.get_port(), e);
                    }
                })
            })
            .collect();

        // 等待所有发送任务完成 (可选，取决于需求)
        join_all(send_futures).await;
        Ok(())
    }

    // 获取指定路径的串口管理器引用
    pub async fn get_port(&self, port_path: &str) -> Option<Arc<SerialPortManager>> {
        let ports = self.ports.lock().await;
        ports.get(port_path).cloned() // cloned() 创建一个新的 Arc 引用
    }

    // 移除前会先关闭该串口并触发其任务的取消
    pub async fn remove_port(&self, port_path: &str) -> Option<Arc<SerialPortManager>> {
        let mut ports = self.ports.lock().await;
        if let Some(manager) = ports.remove(port_path) {
            log::info!("从注册表移除串口 {}", port_path);
            // 触发该串口管理器的取消令牌，通知其任务永久退出
            manager.cancel_tasks().await;
            Some(manager)
        } else {
            None
        }
    }

    pub async fn get_global() -> Arc<SerialPortRegistry> {
        GLOBAL_REGISTRY
            .get_or_init(|| async { SerialPortRegistry::new() })
            .await
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_name() {
        crate::config::init_config();
        let registry = SerialPortRegistry::get_global().await;
        registry.add_task_port("COM1").await;
        // registry.add_task_port("COM2").await;
        registry.add_task_port("COM3").await;
        // registry.add_task_port("COM4").await;
        let _ = registry.add_port("COM1", 256000, 200, 8).await;

        registry.open_all().await;

        tokio::time::sleep(tokio::time::Duration::from_secs(10000)).await;
    }
}
