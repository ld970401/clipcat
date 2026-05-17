#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! 应用程序入口点
//! 在 Windows Release 模式下隐藏控制台窗口，避免出现额外的黑色终端窗口

fn main() {
    clipcat_lib::run()
}
