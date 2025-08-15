use std::{fs::File, io::Write, sync::Arc};

use log::LevelFilter;
use log4rs::{
    append::{console::ConsoleAppender, file::FileAppender},
    config::{Appender, Logger, Root},
    encode::pattern::PatternEncoder,
};
use lazy_static::lazy_static;

use tokio::runtime::Runtime;

lazy_static! {
    pub static ref RUNTIME: Arc<Runtime> = Arc::new(Runtime::new().unwrap());
}

fn init_log() {
    let stdout = ConsoleAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{d} - {m}{n}")))
        .build();

    let file = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{d} - {m}{n}")))
        .build(format!(
            "log/log_{}.log",
            chrono::Utc::now().format("%y-%m-%d")
        ))
        .unwrap();

    let config = log4rs::Config::builder()
        .appender(Appender::builder().build("stdout", Box::new(stdout)))
        .appender(Appender::builder().build("file", Box::new(file)))
        .logger(Logger::builder().build("rocket", LevelFilter::Warn)) // 设置模块的日志级别
        .build(
            Root::builder()
                .appender("stdout")
                .appender("file")
                .build(LevelFilter::Info),
        )
        .unwrap();

    let _ = log4rs::init_config(config);
}

pub fn init_config() {
    init_log();
}

pub fn write_vec_to_file(filename: &str, data: &Vec<String>) {
    let mut file = File::create(filename).unwrap();

    for item in data {
        // 写入文件
        file.write_all(item.as_bytes()).unwrap();
    }

    log::info!("save content to file = {}", filename);
}


pub fn get_runtime() -> Arc<Runtime> {
    return RUNTIME.clone();
}
