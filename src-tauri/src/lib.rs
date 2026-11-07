mod cache;
mod commands;
mod readers;
mod types;
mod utils;

use cache::ClipboardCache;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(ClipboardCache::new())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::read_clipboard,
            commands::get_format_raw_data,
            commands::get_format_hash,
            commands::get_clipboard_hash
        ])
        .run(tauri::generate_context!())
        .expect("启动 Tauri 应用时出错");
}
