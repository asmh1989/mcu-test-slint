use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc; // 如果需要在多个任务间共享 SerialPortManager 实例
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;
use tokio::sync::Mutex; // 用于在异步任务间安全共享可变状态
use tokio::time::{self, Duration};
use tokio_serial::{SerialPortBuilderExt, SerialStream};
use tokio_util::sync::CancellationToken;

// 定义一个结构体来封装接收到的数据
// 移除 port_path 属性，因为数据将在 SerialPortManager 内部处理
#[derive(Debug)]
pub struct ReceivedData {
    pub data: Vec<u8>,
    // 如果需要，可以添加时间戳或其他元数据
}

// 定义一个结构体来管理单个串口
pub struct SerialPortManager {
    port_path: String, // 串口路径，例如 "/dev/ttyUSB0" 或 "COM3"
    baud_rate: u32,
    // 使用 Mutex 保护 SerialStream，使其可以在多个异步任务中被访问和修改
    // Option 允许在串口未打开或断开时为 None
    port: Mutex<Option<SerialStream>>,
    // 通道发送端，用于将接收到的数据发送到内部处理任务
    data_sender: mpsc::Sender<ReceivedData>,
    // 取消令牌，用于通知该串口相关的任务退出 (接收任务和数据处理任务)
    cancel_token: CancellationToken,
    // 添加一个单独的原子标志来表示连接状态，避免阻塞 is_open
    is_connected: AtomicBool,
    // 读取超时时间
    read_timeout: Duration,
    // 数据处理任务的句柄 (可选，如果需要等待任务完成)
    // data_processing_task_handle: Option<tokio::task::JoinHandle<()>>,
}

impl SerialPortManager {
    // 构造函数
    // read_timeout_ms: 读取超时时间，毫秒
    // cancel_token: 用于控制该管理器及其任务生命周期的取消令牌
    // data_channel_buffer_size: 内部数据通道的缓冲区大小
    // enable_auto_receive: 是否启用自动接收任务 (对于SCPI通信通常设为false)
    pub fn new(
        port_path: &str,
        baud_rate: u32,
        read_timeout_ms: u64,
        cancel_token: CancellationToken, // 接受外部提供的取消令牌
        data_channel_buffer_size: usize, // 内部通道缓冲区大小
        enable_auto_receive: bool,       // 是否启用自动接收任务
    ) -> Arc<Self> {
        // 为该串口管理器创建独立的 MPSC 通道
        let (data_sender, data_receiver) = mpsc::channel::<ReceivedData>(data_channel_buffer_size);

        let manager = Self {
            port_path: port_path.to_string(),
            baud_rate,
            port: Mutex::new(None),
            data_sender,
            cancel_token,                         // 使用传入的令牌
            is_connected: AtomicBool::new(false), // 初始化连接状态为 false
            read_timeout: Duration::from_millis(read_timeout_ms), // 设置读取超时时间
                                                  // data_processing_task_handle: None, // 初始化为 None
        };

        // 在创建管理器时启动内部数据处理任务
        // 需要将 manager 包装在 Arc 中才能在 spawned 任务中使用
        let manager_arc = Arc::new(manager);

        // 根据参数决定是否启动自动接收任务
        if enable_auto_receive {
            manager_arc.start_data_processing_task(data_receiver);
            manager_arc.start_receive_task(); // 启动接收任务
        }

        // 返回 Arc<Self> 的克隆
        manager_arc.clone()
    }

    // SCPI通信专用的构造函数 - 禁用自动接收任务
    pub fn new_for_scpi(
        port_path: &str,
        baud_rate: u32,
        cancel_token: CancellationToken,
    ) -> Arc<Self> {
        Self::new(
            port_path,
            baud_rate,
            1000, // SCPI通信默认1秒超时
            cancel_token,
            10,    // 小缓冲区，因为不使用自动接收
            false, // 禁用自动接收任务
        )
    }

    // 检查串口当前是否打开
    // 这个方法现在读取原子标志，避免了对 port Mutex 的锁定
    pub fn is_open(&self) -> bool {
        self.is_connected.load(Ordering::SeqCst) // 原子读取连接状态
    }

    pub fn get_port(&self) -> &str {
        &self.port_path
    }

    // 打开串口
    pub async fn open(&self) -> anyhow::Result<()> {
        let mut port_guard = self.port.lock().await;
        if port_guard.is_some() {
            // 串口已打开
            log::info!("串口 {} 已打开", self.port_path);
            // 确保原子状态正确
            self.is_connected.store(true, Ordering::SeqCst);
            return Ok(());
        }

        log::info!("尝试打开串口: {} @ {}", self.port_path, self.baud_rate);
        // 注意: 在 Windows 上，串口名通常是 "COM1", "COM2" 等
        // 在 Linux 上，通常是 "/dev/ttyUSB0", "/dev/ttyACM0" 等
        // open_native_async 允许设置一些原生参数，但 tokio-serial 的 read 方法本身没有内置超时
        // 我们将在接收任务中通过 tokio::time::timeout 来实现超时
        match tokio_serial::new(&self.port_path, self.baud_rate).open_native_async() {
            Ok(stream) => {
                log::info!("串口打开成功");
                *port_guard = Some(stream);
                self.is_connected.store(true, Ordering::SeqCst); // 打开成功，设置状态为 true

                // 释放锁，然后进行初始化
                drop(port_guard);

                // 执行SCPI初始化流程
                match self.scpi_initialize().await {
                    Ok(_) => {
                        log::info!("串口 {} 初始化成功", self.port_path);
                        Ok(())
                    }
                    Err(e) => {
                        log::error!("串口 {} 初始化失败: {}", self.port_path, e);
                        // 初始化失败，关闭串口
                        self.close().await;
                        Err(anyhow::anyhow!("串口初始化失败: {}", e))
                    }
                }
            }
            Err(e) => {
                log::error!("打开串口失败: {}", e);
                self.is_connected.store(false, Ordering::SeqCst); // 打开失败，设置状态为 false
                Err(anyhow::anyhow!(e))
            }
        }
    }

    // 关闭串口
    pub async fn close(&self) {
        let mut port_guard = self.port.lock().await;
        if port_guard.is_some() {
            log::info!("关闭串口: {}", self.port_path);
            // 丢弃 SerialStream 会自动关闭底层文件句柄
            *port_guard = None;
        }
        self.is_connected.store(false, Ordering::SeqCst); // 设置状态为 false
                                                          // 主动关闭时，触发取消令牌，让接收和数据处理任务退出
                                                          // self.cancel_token.cancel();
    }

    // 发送数据
    pub async fn send(&self, data: &[u8]) -> anyhow::Result<()> {
        // 在发送前快速检查连接状态，避免不必要的锁等待
        if !self.is_connected.load(Ordering::SeqCst) {
            return Err(anyhow::anyhow!("Serial port not open"));
        }

        let mut port_guard = self.port.lock().await;
        if let Some(port) = port_guard.as_mut() {
            log::info!(
                "发送数据 ({}): {}",
                self.port_path,
                String::from_utf8_lossy(data).trim()
            );
            match port.write_all(data).await {
                Ok(_) => Ok(()),
                Err(e) => {
                    log::error!("发送数据 ({}): 失败: {}", self.port_path, e);
                    // 发送失败通常也意味着断开，但为了避免读写任务冲突修改 port 状态，
                    // 主要依赖接收任务的读取错误来标记断开。
                    // 可以在这里也尝试设置 is_connected 为 false，但需要小心同步问题
                    // self.is_connected.store(false, Ordering::SeqCst);
                    Err(anyhow::anyhow!(e))
                }
            }
        } else {
            // 理论上 is_connected 已经是 false 了，但为了健壮性再次返回错误
            Err(anyhow::anyhow!("Serial port not open"))
        }
    }

    // 发送Modbus命令并接收原始字节响应
    pub async fn send_modbus_command(
        &self,
        command: &[u8],
        timeout_ms: u64,
    ) -> anyhow::Result<Vec<u8>> {
        // 检查连接状态
        if !self.is_connected.load(Ordering::SeqCst) {
            return Err(anyhow::anyhow!("Serial port not open"));
        }

        let mut port_guard = self.port.lock().await;
        if let Some(port) = port_guard.as_mut() {
            log::info!("发送Modbus命令 ({}): {:02X?}", self.port_path, command);

            // 发送命令
            match port.write_all(command).await {
                Ok(_) => {}
                Err(e) => {
                    log::error!("发送Modbus命令 ({}): 失败: {}", self.port_path, e);
                    return Err(anyhow::anyhow!("Failed to send command: {}", e));
                }
            }

            // 接收响应
            let mut buffer = vec![0u8; 1024];
            let timeout = Duration::from_millis(timeout_ms);

            // 使用超时读取响应
            match time::timeout(timeout, async {
                let mut response = Vec::new();

                loop {
                    match port.read(&mut buffer).await {
                        Ok(n) if n > 0 => {
                            response.extend_from_slice(&buffer[..n]);

                            // 对于Modbus RTU，检查是否接收到完整帧
                            // 最小帧长度是4字节（地址+功能码+CRC）
                            if n >= 4 {
                                break;
                            }
                        }
                        Ok(_) => {
                            // n == 0 - EOF，连接可能已断开
                            if response.len() >= 4 {
                                break; // 已有数据，可能是正常结束
                            }
                            return Err(anyhow::anyhow!("Connection closed"));
                        }
                        Err(e) => {
                            return Err(anyhow::anyhow!("Read error: {}", e));
                        }
                    }
                }
                Ok::<Vec<u8>, anyhow::Error>(response)
            })
            .await
            {
                Ok(Ok(data)) => {
                    log::info!("接收Modbus响应 ({}): {:02X?}", self.port_path, data);
                    Ok(data)
                }
                Ok(Err(e)) => {
                    log::error!("读取Modbus响应失败 ({}): {}", self.port_path, e);
                    Err(e)
                }
                Err(_) => {
                    log::warn!("读取Modbus响应超时 ({})", self.port_path);
                    Err(anyhow::anyhow!("Response timeout"))
                }
            }
        } else {
            Err(anyhow::anyhow!("Serial port not open"))
        }
    }

    // 启动一个任务来持续接收串口原始数据
    // 这个任务会在后台运行，轮询读取串口数据
    // 任务会检查取消令牌，并在断开时进入等待重连状态
    // pub(crate) 限制此方法只能在当前 crate 中调用，通常由 SerialPortRegistry 调用
    fn start_receive_task(self: &Arc<Self>) {
        let manager = Arc::clone(self); // 克隆 Arc 引用，以便在任务中使用
        let cancel_token = manager.cancel_token.clone(); // 克隆取消令牌
        let read_timeout = manager.read_timeout; // 获取读取超时时间
        let data_sender = manager.data_sender.clone(); // 克隆内部通道发送端

        tokio::spawn(async move {
            let mut buffer = vec![0u8; 1024]; // 接收缓冲区大小
            log::info!("接收任务 ({}): 启动", manager.port_path);
            loop {
                // 在循环开始时检查取消信号
                tokio::select! {
                    _ = cancel_token.cancelled() => {
                        log::info!("接收任务 ({}): 收到取消信号，退出", manager.port_path);
                        break; // 收到取消信号，退出循环
                    }
                    _ = async {} => {} // 这个分支总是 ready
                }

                let mut port_guard = manager.port.lock().await;
                if let Some(port) = port_guard.as_mut() {
                    // log::info!("接收任务 ({}): 尝试读取数据...", manager.port_path);
                    // 使用 tokio::select! 同时监听读取操作、取消信号和超时
                    tokio::select! {
                        _ = cancel_token.cancelled() => {
                            log::info!("接收任务 ({}): 收到取消信号，读取中退出", manager.port_path);
                            break; // 收到取消信号，退出循环
                        }
                        result = time::timeout(read_timeout, port.read(&mut buffer)) => {
                            match result {
                                Ok(Ok(n)) => { // 读取成功
                                    log::debug!("接收任务 ({}): 读取到 {} 字节", manager.port_path, n);
                                    if n > 0 {
                                        // println!("接收任务 ({}): 接收到 {} 字节", manager.port_path, n);
                                        // 通过内部通道发送接收到的数据
                                        if data_sender
                                            .send(ReceivedData {
                                                data: buffer[..n].to_vec(),
                                            })
                                            .await
                                            .is_err()
                                        {
                                            log::info!("接收任务 ({}): 发送数据到内部通道失败，处理者可能已关闭", manager.port_path);
                                            manager.is_connected.store(false, Ordering::SeqCst); // 更新状态
                                            break; // 接收者已丢弃，退出任务
                                        }
                                    } else {
                                        // n == 0 可能发生在流关闭时 (例如 EOF)，在某些平台/驱动上可能表示断开。
                                        log::info!("接收任务 ({}): 读取到 0 字节，可能已断开", manager.port_path);
                                        *port_guard = None; // 标记为断开
                                        manager.is_connected.store(false, Ordering::SeqCst); // 更新状态
                                        // 不在此处 break，循环会继续，进入下面的 else 分支等待重连
                                    }
                                }
                                Ok(Err(e)) => { // 读取操作本身返回错误 (非超时)
                                    log::info!("接收任务 ({}): 读取数据失败: {}", manager.port_path, e);
                                    // 错误表示断开或其他问题
                                    *port_guard = None; // 标记为断开
                                    manager.is_connected.store(false, Ordering::SeqCst); // 更新状态
                                    // 不在此处 break，循环会继续，进入下面的 else 分支等待重连
                                }
                                Err(_) => { // 超时错误
                                    // log::info!("接收任务 ({}): 读取超时", manager.port_path);
                                    // 超时不一定意味着断开，但为了简化逻辑，我们仍然标记为断开并依赖监测任务重连
                                    // 如果需要区分超时和实际断开，需要更复杂的逻辑
                                    // *port_guard = None; // 标记为断开
                                    // manager.is_connected.store(false, Ordering::SeqCst); // 更新状态
                                    // 不在此处 break，循环会继续，进入下面的 else 分支等待重连
                                }
                            }
                        }
                    }
                } else {
                    // 串口未打开，等待一段时间再检查
                    // 监测任务会处理重连尝试
                    // println!("接收任务 ({}): 串口未打开，等待...", manager.port_path);
                    // 在等待时也检查取消信号
                    tokio::select! {
                        _ = cancel_token.cancelled() => {
                            log::info!("接收任务 ({}): 收到取消信号，等待中退出", manager.port_path);
                            break; // 收到取消信号，退出循环
                        }
                        _ = tokio::time::sleep(tokio::time::Duration::from_secs(1)) => {
                            // 等待结束，继续循环检查串口状态
                        }
                    }
                }
            }
            log::info!("接收任务 ({}): 退出", manager.port_path);
        });
    }

    // 启动一个任务来处理从内部通道接收到的数据
    // pub(crate) 限制此方法只能在当前 crate 中调用，通常由 SerialPortRegistry 调用
    fn start_data_processing_task(
        self: &Arc<Self>,
        mut data_receiver: mpsc::Receiver<ReceivedData>,
    ) {
        let manager = Arc::clone(self); // 克隆 Arc 引用
        let cancel_token = manager.cancel_token.clone(); // 克隆取消令牌

        let mut buffer = vec![0u8; 1024]; // 数据处理缓冲区大小
        tokio::spawn(async move {
            log::info!("数据处理任务 ({}): 启动", manager.port_path);
            loop {
                tokio::select! {
                    _ = cancel_token.cancelled() => {
                        log::info!("数据处理任务 ({}): 收到取消信号，退出", manager.port_path);
                        break; // 收到取消信号，退出循环
                    }
                    Some(received) = data_receiver.recv() => {
                        // 在这里处理接收到的数据
                        // log::info!("数据处理任务 ({}): 收到数据: {:?}", manager.port_path, received.data);
                        buffer.extend_from_slice(&received.data);

                        // }
                        // 根据需要添加您的数据处理逻辑
                    }
                }
            }
            log::info!("数据处理任务 ({}): 退出", manager.port_path);
        });
    }

    /// SCPI串口初始化流程
    /// 执行标准的SCPI设备初始化步骤
    async fn scpi_initialize(&self) -> anyhow::Result<()> {
        log::info!("开始SCPI初始化流程: {}", self.port_path);
        Ok(())
    }

    // 触发取消令牌，通知该串口相关的任务退出
    // pub(crate) 限制此方法只能在当前 crate 中调用，通常由 SerialPortRegistry 调用
    pub async fn cancel_tasks(&self) {
        log::info!("串口 {} 触发取消令牌，通知任务退出...", self.port_path);
        self.close().await; // 关闭串口并清理状态
        self.cancel_token.cancel();
    }
}

#[cfg(test)]
mod tests {
    use crate::serial::base::SerialPortManager;

    #[test]
    fn test_ports() {
        // 测试当前系统可用的串口列表
        let ports = tokio_serial::available_ports().unwrap();
        println!("当前系统可用的串口列表: {:?}", ports);
    }

    #[tokio::test]
    async fn test_recv_data() {
        crate::config::init_config();
        // 测试接收数据的功能
        let port_path = "COM2"; // 替换为实际的串口路径
        let baud_rate = 256000;
        let read_timeout_ms = 200; // 1秒超时
        let cancel_token = tokio_util::sync::CancellationToken::new();
        let data_channel_buffer_size = 100; // 内部通道缓冲区大小

        // 创建 SerialPortManager 实例
        let manager = SerialPortManager::new(
            port_path,
            baud_rate,
            read_timeout_ms,
            cancel_token.clone(),
            data_channel_buffer_size,
            true, // 启用自动接收任务用于测试
        );

        // 打开串口
        manager.open().await.unwrap();

        let frame = [0u8; 64]; // 示例帧数据

        manager.send(&frame).await.expect("发送数据失败");

        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

        manager.send(&frame).await.expect("发送数据失败");

        // 等待一段时间以便接收数据 (这里可以模拟发送数据到串口)
        tokio::time::sleep(tokio::time::Duration::from_secs(500)).await;

        // 关闭串口
        manager.close().await;
    }
}
