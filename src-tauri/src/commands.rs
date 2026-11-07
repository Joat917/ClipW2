use crate::cache::ClipboardCache;
use crate::readers;
use crate::readers::is_non_global_format;
use crate::types::Output;
use crate::utils::{calculate_hash, category_order};
use std::time::Duration;
use tauri::ipc::Response;
use windows::Win32::System::DataExchange::*;

unsafe fn open_clipboard_with_retry() -> bool {
    for _ in 0..5 {
        if OpenClipboard(None).is_ok() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

#[tauri::command]
pub fn read_clipboard(
    cache: tauri::State<'_, ClipboardCache>,
) -> std::result::Result<String, String> {
    unsafe {
        if !open_clipboard_with_retry() {
            return Ok(serde_json::to_string(&serde_json::json!({
                "error": "无法打开剪贴板，可能被其他程序占用"
            }))
            .unwrap());
        }

        let mut results = Vec::new();
        let mut format_ids = Vec::new();
        let mut fmt = EnumClipboardFormats(0);
        while fmt != 0 {
            format_ids.push(fmt);
            fmt = EnumClipboardFormats(fmt);
        }

        if format_ids.is_empty() {
            CloseClipboard().ok();
            return Ok(serde_json::to_string(&Output { formats: vec![] }).unwrap());
        }

        let mut data_map = cache.data.lock().unwrap();
        let mut hash_map = cache.hashes.lock().unwrap();
        data_map.clear();
        hash_map.clear();

        for id in &format_ids {
            match readers::read_format(*id) {
                Ok((entry, raw_data)) => {
                    if let Some(data) = raw_data {
                        let hash = calculate_hash(&data);
                        hash_map.insert(*id, hash);
                        data_map.insert(*id, data);
                    }
                    results.push(entry);
                }
                Err(_) => {}
            }
        }
        drop(data_map);
        drop(hash_map);

        CloseClipboard().ok();

        results.sort_by(|a, b| {
            category_order(&a.category)
                .cmp(&category_order(&b.category))
                .then(a.format_id.cmp(&b.format_id))
        });

        Ok(serde_json::to_string(&Output { formats: results }).unwrap())
    }
}

#[tauri::command]
pub fn get_format_raw_data(
    format_id: u32,
    cache: tauri::State<'_, ClipboardCache>,
) -> std::result::Result<Response, String> {
    let data_map = cache.data.lock().unwrap();
    match data_map.get(&format_id) {
        Some(data) => Ok(Response::new(data.clone())),
        None => Err(format!("格式 #{} 的数据不可用", format_id)),
    }
}

#[tauri::command]
pub fn get_format_hash(
    format_id: u32,
    cache: tauri::State<'_, ClipboardCache>,
) -> std::result::Result<Option<String>, String> {
    let hash_map = cache.hashes.lock().unwrap();
    Ok(hash_map.get(&format_id).cloned())
}

#[tauri::command]
pub fn get_clipboard_hash() -> std::result::Result<String, String> {
    use windows::Win32::{Foundation::HGLOBAL, System::Memory::*};

    unsafe {
        if !open_clipboard_with_retry() {
            return Ok(String::new());
        }

        let mut hash_parts = Vec::new();
        let mut fmt = EnumClipboardFormats(0);
        while fmt != 0 {
            let size = if is_non_global_format(fmt) {
                0
            } else {
                GetClipboardData(fmt)
                    .ok()
                    .and_then(|h| {
                        if h.is_invalid() {
                            None
                        } else {
                            let hmem = HGLOBAL(h.0);
                            let size = GlobalSize(hmem);
                            if size == 0 { None } else { Some(size) }
                        }
                    })
                    .unwrap_or(0)
            };
            hash_parts.push(format!("{}:{}", fmt, size));
            fmt = EnumClipboardFormats(fmt);
        }

        CloseClipboard().ok();
        Ok(calculate_hash(hash_parts.join("|").as_bytes()))
    }
}
