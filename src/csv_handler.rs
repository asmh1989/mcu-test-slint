use anyhow::{Result, anyhow};
use lazy_static::lazy_static;
use rfd::FileDialog;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

/// CSV文件中的寄存器记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRecord {
    #[serde(rename = "Page_Addr")]
    pub page_addr: String,
    #[serde(rename = "Register")]
    pub register: String,
    #[serde(rename = "R_W")]
    pub r_w: String,
    #[serde(rename = "Value")]
    pub value: String,
    /// 额外的写入值字段，默认为空
    #[serde(skip)]
    pub w_value: Option<String>,
}

impl RegisterRecord {
    /// 创建新的寄存器记录
    pub fn new(page_addr: String, register: String, r_w: String, value: String) -> Self {
        Self {
            page_addr,
            register,
            r_w,
            value,
            w_value: None,
        }
    }

    /// 设置写入值
    pub fn set_w_value(&mut self, w_value: Option<String>) {
        self.w_value = w_value;
    }

    /// 获取页地址的数值表示
    pub fn get_page_addr_value(&self) -> Result<u32> {
        if self.page_addr.starts_with("0x") || self.page_addr.starts_with("0X") {
            u32::from_str_radix(&self.page_addr[2..], 16)
                .map_err(|e| anyhow!("无法解析页地址 {}: {}", self.page_addr, e))
        } else {
            self.page_addr
                .parse::<u32>()
                .map_err(|e| anyhow!("无法解析页地址 {}: {}", self.page_addr, e))
        }
    }
}

lazy_static! {
    pub static ref REGISTER_DATA: Mutex<HashMap<String, Vec<RegisterRecord>>> =
        Mutex::new(HashMap::new());
}
/// CSV文件处理器
pub struct CsvHandler;

impl CsvHandler {
    /// 打开文件选择对话框
    pub fn select_csv_file() -> Option<std::path::PathBuf> {
        FileDialog::new()
            .add_filter("CSV Files", &["csv"])
            .set_title("选择CSV文件")
            .pick_file()
    }

    /// 解析CSV文件
    pub fn parse_csv_file(file_path: &std::path::Path) -> Result<Vec<RegisterRecord>> {
        let mut reader = csv::Reader::from_path(file_path)?;
        let mut records = Vec::new();

        for result in reader.deserialize() {
            let record: RegisterRecord = result?;
            records.push(record);
        }

        Ok(records)
    }

    /// 将解析的数据存储到全局变量中
    pub fn store_to_global(records: Vec<RegisterRecord>) -> Result<()> {
        let mut global_data = REGISTER_DATA
            .lock()
            .map_err(|e| anyhow!("无法获取全局数据锁: {}", e))?;

        // 清空现有数据
        global_data.clear();

        // 按页地址分组存储
        for record in records {
            let page_addr = record.page_addr.clone();
            global_data
                .entry(page_addr)
                .or_insert_with(Vec::new)
                .push(record);
        }

        Ok(())
    }

    /// 获取所有数据的表格字符串表示
    pub fn get_table_string() -> Result<String> {
        let global_data = REGISTER_DATA
            .lock()
            .map_err(|e| anyhow!("无法获取全局数据锁: {}", e))?;

        if global_data.is_empty() {
            return Ok("暂无数据".to_string());
        }

        let mut table = String::new();

        // 表头
        table.push_str("页地址\t\t寄存器\t\t读写\t\t值\t\t写入值\n");
        table.push_str("-----------------------------------------------------------\n");

        // 按页地址排序
        let mut sorted_pages: Vec<_> = global_data.keys().collect();
        sorted_pages.sort();

        for page_addr in sorted_pages {
            if let Some(records) = global_data.get(page_addr) {
                for record in records {
                    let w_value = record.w_value.as_deref().unwrap_or("");
                    table.push_str(&format!(
                        "{}\t\t{}\t\t{}\t\t{}\t\t{}\n",
                        record.page_addr, record.register, record.r_w, record.value, w_value
                    ));
                }
            }
        }

        Ok(table)
    }

    /// 获取Slint标准表格数据
    pub fn get_slint_table_data() -> Result<Vec<Vec<slint::SharedString>>> {
        let global_data = REGISTER_DATA
            .lock()
            .map_err(|e| anyhow!("无法获取全局数据锁: {}", e))?;

        if global_data.is_empty() {
            return Ok(vec![]);
        }

        let mut table_data = Vec::new();

        // 按页地址排序
        let mut sorted_pages: Vec<_> = global_data.keys().collect();
        sorted_pages.sort();

        for page_addr in sorted_pages {
            if let Some(records) = global_data.get(page_addr) {
                for record in records {
                    let w_value = record.w_value.as_deref().unwrap_or("");
                    let row = vec![
                        record.page_addr.clone().into(),
                        record.register.clone().into(),
                        record.r_w.clone().into(),
                        record.value.clone().into(),
                        w_value.to_string().into(),
                    ];
                    table_data.push(row);
                }
            }
        }

        Ok(table_data)
    }

    /// 根据页地址获取寄存器记录
    pub fn get_records_by_page(page_addr: &str) -> Result<Vec<RegisterRecord>> {
        let global_data = REGISTER_DATA
            .lock()
            .map_err(|e| anyhow!("无法获取全局数据锁: {}", e))?;

        Ok(global_data.get(page_addr).cloned().unwrap_or_default())
    }

    /// 更新寄存器的写入值
    pub fn update_w_value(page_addr: &str, register: &str, w_value: Option<String>) -> Result<()> {
        let mut global_data = REGISTER_DATA
            .lock()
            .map_err(|e| anyhow!("无法获取全局数据锁: {}", e))?;

        if let Some(records) = global_data.get_mut(page_addr) {
            for record in records.iter_mut() {
                if record.register == register {
                    record.w_value = w_value;
                    return Ok(());
                }
            }
        }

        Err(anyhow!("未找到指定的寄存器: {}:{}", page_addr, register))
    }

    /// 获取所有页地址
    pub fn get_all_page_addresses() -> Result<Vec<String>> {
        let global_data = REGISTER_DATA
            .lock()
            .map_err(|e| anyhow!("无法获取全局数据锁: {}", e))?;

        let mut pages: Vec<String> = global_data.keys().cloned().collect();
        pages.sort();
        Ok(pages)
    }

    /// 清空所有数据
    pub fn clear_all_data() -> Result<()> {
        let mut global_data = REGISTER_DATA
            .lock()
            .map_err(|e| anyhow!("无法获取全局数据锁: {}", e))?;

        global_data.clear();
        Ok(())
    }

    /// 完整的文件读取流程
    pub fn read_csv_file() -> Result<String> {
        // 1. 选择文件
        let file_path = Self::select_csv_file().ok_or_else(|| anyhow!("未选择文件"))?;

        log::info!("选择的文件: {:?}", file_path);

        // 2. 解析CSV文件
        let records = Self::parse_csv_file(&file_path)?;
        log::info!("解析到 {} 条记录", records.len());

        // 3. 存储到全局变量
        Self::store_to_global(records)?;

        // 4. 生成表格字符串
        let table_string = Self::get_table_string()?;

        Ok(table_string)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_csv() {
        // 创建临时CSV文件
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "Page_Addr,Register,R_W,Value").unwrap();
        writeln!(temp_file, "0x0000,CHIPID,R,0x72").unwrap();
        writeln!(temp_file, "0x0001,REVID,R,0x05").unwrap();

        // 解析文件
        let records = CsvHandler::parse_csv_file(temp_file.path()).unwrap();

        assert_eq!(records.len(), 2);
        assert_eq!(records[0].page_addr, "0x0000");
        assert_eq!(records[0].register, "CHIPID");
        assert_eq!(records[0].r_w, "R");
        assert_eq!(records[0].value, "0x72");
        assert_eq!(records[0].w_value, None);
    }

    #[test]
    fn test_store_and_retrieve() {
        // 清空数据
        CsvHandler::clear_all_data().unwrap();

        // 创建测试数据
        let records = vec![
            RegisterRecord::new(
                "0x0000".to_string(),
                "CHIPID".to_string(),
                "R".to_string(),
                "0x72".to_string(),
            ),
            RegisterRecord::new(
                "0x0000".to_string(),
                "REVID".to_string(),
                "R".to_string(),
                "0x05".to_string(),
            ),
            RegisterRecord::new(
                "0x1000".to_string(),
                "PAGE0_CHANNEL_POWERDOWN".to_string(),
                "RW".to_string(),
                "0x00".to_string(),
            ),
        ];

        // 存储数据
        CsvHandler::store_to_global(records).unwrap();

        // 检索数据
        let page_0000_records = CsvHandler::get_records_by_page("0x0000").unwrap();
        assert_eq!(page_0000_records.len(), 2);

        let page_1000_records = CsvHandler::get_records_by_page("0x1000").unwrap();
        assert_eq!(page_1000_records.len(), 1);

        // 获取所有页地址
        let all_pages = CsvHandler::get_all_page_addresses().unwrap();
        println!("实际页地址: {:?}", all_pages);
        assert_eq!(all_pages.len(), 2);
        assert!(all_pages.contains(&"0x0000".to_string()));
        assert!(all_pages.contains(&"0x1000".to_string()));
    }

    #[test]
    fn test_update_w_value() {
        // 清空数据
        CsvHandler::clear_all_data().unwrap();

        // 创建测试数据
        let records = vec![RegisterRecord::new(
            "0x0000".to_string(),
            "CHIPID".to_string(),
            "R".to_string(),
            "0x72".to_string(),
        )];

        // 存储数据
        CsvHandler::store_to_global(records).unwrap();

        // 更新写入值
        CsvHandler::update_w_value("0x0000", "CHIPID", Some("0x80".to_string())).unwrap();

        // 验证更新
        let updated_records = CsvHandler::get_records_by_page("0x0000").unwrap();
        assert_eq!(updated_records[0].w_value, Some("0x80".to_string()));
    }
}
