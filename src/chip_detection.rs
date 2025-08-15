use std::error::Error;
use std::sync::Arc;

use crate::serial::base::SerialPortManager;
use crate::serial::modbus::{ModbusFrame, RegisterType};

// 芯片类型枚举
#[derive(Debug, Clone, PartialEq)]
pub enum ChipType {
    MALD,
    MATA,
    Unknown,
}

impl ChipType {
    pub fn to_string(&self) -> String {
        match self {
            Self::MALD => "MALD".to_string(),
            Self::MATA => "MATA".to_string(),
            Self::Unknown => "未知".to_string(),
        }
    }
}

// 异步芯片检测函数
pub async fn detect_chip_type(
    port_manager: Arc<SerialPortManager>, 
    slave_address: u8, 
    page40_reg0_address: u16
) -> Result<ChipType, Box<dyn Error + Send + Sync>> {
    // 构建 Modbus RTU 读取命令
    let frame = ModbusFrame::new_read_request(
        slave_address,
        RegisterType::HoldingRegister,
        page40_reg0_address,
        1, // 读取1个寄存器
    )?;
    
    let command = frame.to_bytes();
    log::info!("发送芯片检测命令: {:02X?}", command);
    
    // 发送命令并接收响应
    let response = port_manager.send_and_receive(&command, 1000).await?;
    log::info!("接收到响应: {:02X?}", response);
    
    // 解析响应
    let response_frame = ModbusFrame::from_bytes(&response)?;
    
    let data = response_frame.get_data();
    if data.len() >= 2 {
        let value = u16::from_be_bytes([data[1], data[2]]);
        log::info!("读取到寄存器值: 0x{:04X}", value);
        
        // 根据地址和值判断芯片类型
        match page40_reg0_address {
            0x4000 => { // 芯片1
                match value {
                    0x1C | 0x1D => Ok(ChipType::MALD),
                    _ => Ok(ChipType::Unknown),
                }
            },
            0xC000 => { // 芯片2
                match value {
                    0x10 | 0x11 => Ok(ChipType::MATA),
                    _ => Ok(ChipType::Unknown),
                }
            },
            _ => Ok(ChipType::Unknown),
        }
    } else {
        log::warn!("响应数据长度不足");
        Ok(ChipType::Unknown)
    }
}

// 检测两个芯片的类型
pub async fn detect_all_chips(port_manager: Arc<SerialPortManager>) -> (ChipType, ChipType) {
    let mut chip1_type = ChipType::Unknown;
    let mut chip2_type = ChipType::Unknown;
    
    // 检测芯片1 (地址 0x4000)
    match detect_chip_type(port_manager.clone(), 0x01, 0x4000).await {
        Ok(chip_type) => {
            chip1_type = chip_type;
            log::info!("芯片1检测结果: {:?}", chip1_type);
        },
        Err(e) => {
            log::error!("芯片1检测失败: {}", e);
        }
    }
    
    // 等待一段时间再检测芯片2
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // 检测芯片2 (地址 0xC000)
    match detect_chip_type(port_manager.clone(), 0x01, 0xC000).await {
        Ok(chip_type) => {
            chip2_type = chip_type;
            log::info!("芯片2检测结果: {:?}", chip2_type);
        },
        Err(e) => {
            log::error!("芯片2检测失败: {}", e);
        }
    }
    
    (chip1_type, chip2_type)
}
