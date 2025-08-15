use crc::{Crc, CRC_16_MODBUS};
use thiserror::Error;

// 定义 Modbus 错误类型
#[derive(Error, Debug)]
pub enum ModbusError {
    #[error("CRC校验失败: 计算值 {calculated:04X}, 接收值 {received:04X}")]
    CrcMismatch { calculated: u16, received: u16 },
    
    #[error("Modbus异常响应: 功能码 0x{code:02X}, 错误码 0x{error:02X}")]
    ExceptionResponse { code: u8, error: u8 },
    
    #[error("数据长度无效: 预期 {expected}, 实际 {actual}")]
    InvalidLength { expected: usize, actual: usize },
    
    #[error("功能码不匹配: 预期 0x{expected:02X}, 实际 0x{actual:02X}")]
    FunctionCodeMismatch { expected: u8, actual: u8 },
    
    #[error("IO错误: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("从站地址不匹配: 预期 {expected}, 实际 {actual}")]
    AddressMismatch { expected: u8, actual: u8 },
}

// 定义 Modbus 寄存器类型
#[derive(Debug, Clone, Copy)]
pub enum RegisterType {
    Coil,
    DiscreteInput,
    HoldingRegister,
    InputRegister,
}

impl RegisterType {
    fn read_code(&self) -> u8 {
        match self {
            Self::Coil => 0x01,
            Self::DiscreteInput => 0x02,
            Self::HoldingRegister => 0x03,
            Self::InputRegister => 0x04,
        }
    }
    
    fn write_code(&self) -> u8 {
        match self {
            Self::Coil => 0x05,
            Self::HoldingRegister => 0x06,
            _ => panic!("不支持写入该类型寄存器"),
        }
    }
}

// Modbus-RTU 帧结构体
#[derive(Debug)]
pub struct ModbusFrame {
    slave_address: u8,
    function_code: u8,
    data: Vec<u8>,
}

impl ModbusFrame {
    const MIN_FRAME_LENGTH: usize = 4; // 地址+功能码+CRC
    
    // 创建新帧
    pub fn new(slave_address: u8, function_code: u8, data: Vec<u8>) -> Self {
        Self {
            slave_address,
            function_code,
            data,
        }
    }
    
    // 计算帧CRC
    fn calculate_crc(&self) -> u16 {
        let crc_alg = Crc::<u16>::new(&CRC_16_MODBUS);
        let mut digest = crc_alg.digest();
        
        digest.update(&[self.slave_address]);
        digest.update(&[self.function_code]);
        digest.update(&self.data);
        
        digest.finalize()
    }
    
    // 序列化为字节流
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut frame = Vec::with_capacity(self.data.len() + 4);
        
        frame.push(self.slave_address);
        frame.push(self.function_code);
        frame.extend_from_slice(&self.data);
        
        let crc = self.calculate_crc();
        frame.push((crc & 0xFF) as u8); // CRC低字节
        frame.push((crc >> 8) as u8);   // CRC高字节
        
        frame
    }
    
    // 从字节流反序列化
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ModbusError> {
        if bytes.len() < Self::MIN_FRAME_LENGTH {
            return Err(ModbusError::InvalidLength {
                expected: Self::MIN_FRAME_LENGTH,
                actual: bytes.len(),
            });
        }
        
        let crc_position = bytes.len() - 2;
        let (frame_bytes, crc_bytes) = bytes.split_at(crc_position);
        
        // 校验CRC
        let crc = u16::from_le_bytes([crc_bytes[0], crc_bytes[1]]);
        let calc_crc = {
            let crc_alg = Crc::<u16>::new(&CRC_16_MODBUS);
            let mut rr = crc_alg.digest_with_initial(0xFFFF);
            rr.update(frame_bytes);
            rr.finalize()
        };
        
        if calc_crc != crc {
            return Err(ModbusError::CrcMismatch {
                calculated: calc_crc,
                received: crc,
            });
        }
        
        // 检查异常响应
        if frame_bytes[1] & 0x80 != 0 {
            return Err(ModbusError::ExceptionResponse {
                code: frame_bytes[1] & 0x7F,
                error: if frame_bytes.len() > 2 { frame_bytes[2] } else { 0 },
            });
        }
        
        Ok(Self {
            slave_address: frame_bytes[0],
            function_code: frame_bytes[1],
            data: frame_bytes[2..].to_vec(),
        })
    }
}
