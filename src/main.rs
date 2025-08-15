#![allow(dead_code)]
// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;

mod config;
mod serial;

slint::include_modules!();

fn main() -> Result<(), Box<dyn Error>> {
    config::init_config();

    log::info!("init_config");

    let ui = AppWindow::new()?;

    ui.run()?;

    Ok(())
}
