use anyhow::{Result, anyhow};
use lazy_static::lazy_static;
use rfd::FileDialog;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::Mutex;

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
    pub static ref REGISTER_DATA: Mutex<HashMap<String, RegisterRecord>> =
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
    pub async fn store_to_global(records: Vec<RegisterRecord>) -> Result<()> {
        let mut global_data = REGISTER_DATA.lock().await;

        // 清空现有数据
        global_data.clear();

        // 一对一存储，如果存在则覆盖
        for record in records {
            let key = format!("{}", record.page_addr);
            global_data.insert(key, record);
        }

        Ok(())
    }

    /// 获取所有数据的表格字符串表示
    pub async fn get_table_string() -> Result<String> {
        let global_data = REGISTER_DATA.lock().await;

        if global_data.is_empty() {
            return Ok("暂无数据".to_string());
        }

        let mut table = String::new();

        // 表头
        table.push_str("页地址\t\t寄存器\t\t读写\t\t值\t\t写入值\n");
        table.push_str("-----------------------------------------------------------\n");

        // 按key排序，然后显示记录
        let mut sorted_keys: Vec<_> = global_data.keys().collect();
        sorted_keys.sort();

        for key in sorted_keys {
            if let Some(record) = global_data.get(key) {
                let w_value = record.w_value.as_deref().unwrap_or("");
                table.push_str(&format!(
                    "{}\t\t{}\t\t{}\t\t{}\t\t{}\n",
                    record.page_addr, record.register, record.r_w, record.value, w_value
                ));
            }
        }

        Ok(table)
    }

    /// 获取Slint标准表格数据
    pub async fn get_slint_table_data() -> Result<Vec<Vec<slint::SharedString>>> {
        let global_data = REGISTER_DATA.lock().await;

        if global_data.is_empty() {
            return Ok(vec![]);
        }

        let mut table_data = Vec::new();

        // 按key排序
        let mut sorted_keys: Vec<_> = global_data.keys().collect();
        sorted_keys.sort();

        for key in sorted_keys {
            if let Some(record) = global_data.get(key) {
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

        Ok(table_data)
    }

    /// 根据页地址获取寄存器记录
    pub async fn get_records_by_page(page_addr: &str) -> Result<Vec<RegisterRecord>> {
        let global_data = REGISTER_DATA.lock().await;

        let mut records = Vec::new();
        for (_key, record) in global_data.iter() {
            if record.page_addr == page_addr {
                records.push(record.clone());
            }
        }

        Ok(records)
    }

    /// 更新寄存器的写入值
    pub async fn update_w_value(
        page_addr: &str,
        register: &str,
        w_value: Option<String>,
    ) -> Result<()> {
        let mut global_data = REGISTER_DATA.lock().await;

        let key = format!("{}", page_addr);
        if let Some(record) = global_data.get_mut(&key) {
            record.w_value = w_value;
            Ok(())
        } else {
            Err(anyhow!("未找到指定的寄存器: {}:{}", page_addr, register))
        }
    }

    /// 获取所有寄存器记录
    pub async fn get_all_records() -> Result<Vec<RegisterRecord>> {
        let global_data = REGISTER_DATA.lock().await;

        let mut records: Vec<RegisterRecord> = global_data.values().cloned().collect();

        // 根据 page_addr 排序
        records.sort_by_key(|record| record.page_addr.clone());

        Ok(records)
    }

    /// 获取所有页地址
    pub async fn get_all_page_addresses() -> Result<Vec<String>> {
        let global_data = REGISTER_DATA.lock().await;

        let mut pages: Vec<String> = global_data
            .values()
            .map(|record| record.page_addr.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        pages.sort();
        Ok(pages)
    }

    /// 清空所有数据
    pub async fn clear_all_data() -> Result<()> {
        let mut global_data = REGISTER_DATA.lock().await;

        global_data.clear();
        Ok(())
    }

    /// 完整的文件读取流程
    pub async fn read_csv_file() -> Result<String> {
        // 1. 选择文件
        let file_path = Self::select_csv_file().ok_or_else(|| anyhow!("未选择文件"))?;

        log::info!("选择的文件: {:?}", file_path);

        // 2. 解析CSV文件
        let records = Self::parse_csv_file(&file_path)?;
        log::info!("解析到 {} 条记录", records.len());

        // 3. 存储到全局变量
        Self::store_to_global(records).await?;

        // 4. 生成表格字符串
        let table_string = Self::get_table_string().await?;

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

    #[tokio::test]
    async fn test_store_and_retrieve() {
        // 清空数据
        CsvHandler::clear_all_data().await.unwrap();

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
        CsvHandler::store_to_global(records).await.unwrap();

        // 检索数据
        let page_0000_records = CsvHandler::get_records_by_page("0x0000").await.unwrap();
        assert_eq!(page_0000_records.len(), 2);

        let page_1000_records = CsvHandler::get_records_by_page("0x1000").await.unwrap();
        assert_eq!(page_1000_records.len(), 1);

        // 获取所有页地址
        let all_pages = CsvHandler::get_all_page_addresses().await.unwrap();
        println!("实际页地址: {:?}", all_pages);
        assert_eq!(all_pages.len(), 2);
        assert!(all_pages.contains(&"0x0000".to_string()));
        assert!(all_pages.contains(&"0x1000".to_string()));
    }

    #[tokio::test]
    async fn test_update_w_value() {
        // 清空数据
        CsvHandler::clear_all_data().await.unwrap();

        // 创建测试数据
        let records = vec![RegisterRecord::new(
            "0x0000".to_string(),
            "CHIPID".to_string(),
            "R".to_string(),
            "0x72".to_string(),
        )];

        // 存储数据
        CsvHandler::store_to_global(records).await.unwrap();

        // 更新写入值
        CsvHandler::update_w_value("0x0000", "CHIPID", Some("0x80".to_string()))
            .await
            .unwrap();

        // 验证更新
        let updated_records = CsvHandler::get_records_by_page("0x0000").await.unwrap();
        assert_eq!(updated_records[0].w_value, Some("0x80".to_string()));
    }
}
