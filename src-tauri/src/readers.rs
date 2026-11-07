use crate::types::{Content, Entry};
use crate::utils::{
    classify_text, decode_text_bytes, hex_dump, is_ascii, is_printable_text, safe_truncate,
};
use std::slice;
use windows::Win32::{
    Foundation::{HANDLE, HGLOBAL},
    System::{DataExchange::*, Memory::*},
    UI::Shell::{DragQueryFileW, HDROP},
};

pub fn is_non_global_format(id: u32) -> bool {
    matches!(id, 2 | 14 | 15)
}

enum ClipboardHandle {
    Global(HANDLE),
    Bitmap(HANDLE),
    EnhancedMetafile,
    Drop(HANDLE),
}

fn get_clipboard_handle(id: u32) -> windows::core::Result<ClipboardHandle> {
    let h_data = unsafe { GetClipboardData(id)? };
    if h_data.is_invalid() {
        return Err(windows::core::Error::from_win32());
    }

    Ok(match id {
        2 => ClipboardHandle::Bitmap(h_data),
        14 => ClipboardHandle::EnhancedMetafile,
        15 => ClipboardHandle::Drop(h_data),
        _ => ClipboardHandle::Global(h_data),
    })
}

pub unsafe fn read_format(id: u32) -> windows::core::Result<(Entry, Option<Vec<u8>>)> {
    let name = get_format_name(id);
    let desc = get_format_description(id);
    
    match get_clipboard_handle(id) {
        Ok(ClipboardHandle::Bitmap(h)) => {
            let entry = handle_bitmap(h, name, id, desc)?;
            Ok((entry, None))
        }
        Ok(ClipboardHandle::EnhancedMetafile) => {
            let entry = handle_emf(name, id, desc)?;
            Ok((entry, None))
        }
        Ok(ClipboardHandle::Drop(h)) => {
            let entry = handle_file_list(h, name, id, desc)?;
            Ok((entry, None))
        }
        Ok(ClipboardHandle::Global(h)) => {
            let raw = read_global_memory(h);
            let data = raw.as_deref().unwrap_or_default();

            let entry = match id {
                1 => build_text_entry(data, name, id, "text", "纯文本 (ANSI)", desc),
                4 => build_text_entry(data, name, id, "text", "SYLK (符号链接)", desc),
                5 => build_text_entry(data, name, id, "text", "DIF (数据交换)", desc),
                7 => build_text_entry(data, name, id, "text", "纯文本 (OEM)", desc),
                8 => handle_dib(data, name, id, desc),
                13 => build_text_entry_utf16(data, name, id, desc),
                17 => handle_dib_v5(data, name, id, desc),
                _ => try_build_entry(data, name.clone(), id, &name, desc),
            }?;

            Ok((entry, raw))
        }
        Err(_) => {
            Ok((
                Entry {
                    format_name: name,
                    format_id: id,
                    category: "empty".into(),
                    type_label: "空".into(),
                    encoding: None,
                    description: desc,
                    content: None,
                    width: None,
                    height: None,
                    size: None,
                    preview: Some("(无数据)".into()),
                },
                None,
            ))
        }
    }
}

unsafe fn read_global_memory(h_global: HANDLE) -> Option<Vec<u8>> {
    let hmem = HGLOBAL(h_global.0);
    let p_mem = GlobalLock(hmem);
    if p_mem.is_null() {
        return None;
    }
    let size = GlobalSize(hmem);
    if size == 0 {
        GlobalUnlock(hmem).ok();
        return None;
    }
    let data = slice::from_raw_parts(p_mem as *const u8, size).to_vec();
    GlobalUnlock(hmem).ok();
    Some(data)
}

fn build_text_entry(
    data: &[u8],
    name: String,
    id: u32,
    default_cat: &str,
    type_label: &str,
    description: Option<String>,
) -> windows::core::Result<Entry> {
    if data.is_empty() {
        return Ok(empty_entry(name, id, default_cat, type_label, description));
    }

    let (text, encoding_name) = decode_text_bytes(data);
    let encoding = if is_ascii(data) { "ASCII" } else { encoding_name };
    Ok(make_text_entry(
        text,
        name,
        id,
        default_cat,
        type_label,
        Some(encoding),
        description,
        data.len(),
    ))
}

fn build_text_entry_utf16(
    data: &[u8],
    name: String,
    id: u32,
    description: Option<String>,
) -> windows::core::Result<Entry> {
    if data.is_empty() {
        return Ok(empty_entry(name, id, "text", "纯文本 (Unicode)", description));
    }
    let text = if data.len() >= 2 {
        let u16_data: Vec<u16> = data
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16_lossy(&u16_data)
            .trim_end_matches('\0')
            .to_string()
    } else {
        String::new()
    };
    Ok(make_text_entry(
        text,
        name,
        id,
        "text",
        "纯文本 (Unicode)",
        Some("UTF-16LE"),
        description,
        data.len(),
    ))
}

fn build_labeled_text_entry(
    data: &[u8],
    name: String,
    id: u32,
    category: &str,
    type_prefix: &str,
    label_hint: &str,
    description: Option<String>,
) -> Entry {
    let (text, enc) = decode_text_bytes(data);
    let encoding = if is_ascii(data) { "ASCII" } else { enc };
    let type_label = format!("{} ({})", type_prefix, label_hint);
    make_text_entry(
        text,
        name,
        id,
        category,
        &type_label,
        Some(encoding),
        description,
        data.len(),
    )
}

fn try_build_entry(
    data: &[u8],
    name: String,
    id: u32,
    label_hint: &str,
    description: Option<String>,
) -> windows::core::Result<Entry> {
    if data.is_empty() {
        return Ok(empty_entry(name, id, "empty", label_hint, description));
    }

    let lower_name = label_hint.to_lowercase();
    if lower_name.contains("html") {
        return Ok(build_labeled_text_entry(
            data, name, id, "html", "HTML", label_hint, description,
        ));
    }
    if lower_name.contains("rtf") {
        return Ok(build_labeled_text_entry(
            data, name, id, "rtf", "RTF", label_hint, description,
        ));
    }
    if lower_name == "png" {
        return Ok(image_entry(
            name, id, &format!("PNG ({})", label_hint), description,
            None, None, data.len(), format!("PNG, {} 字节", data.len()),
        ));
    }

    let (text, enc) = decode_text_bytes(data);
    if is_printable_text(&text) {
        let type_label = format!("文本 ({})", label_hint);
        Ok(make_text_entry(
            text,
            name,
            id,
            "text",
            &type_label,
            Some(enc),
            description,
            data.len(),
        ))
    } else {
        Ok(binary_entry(
            name, id, &format!("二进制 ({})", label_hint), description,
            data.len(), &hex_dump(data, data.len()),
        ))
    }
}

fn make_text_entry(
    text: String,
    name: String,
    id: u32,
    default_cat: &str,
    type_label: &str,
    encoding: Option<&str>,
    description: Option<String>,
    raw_len: usize,
) -> Entry {
    let category = classify_text(&text);
    let category = if category == "text" {
        default_cat.to_string()
    } else {
        category
    };
    let preview = safe_truncate(&text, 500);
    let content = if text.len() > 100000 {
        format!("{}...", safe_truncate(&text, 100000))
    } else {
        text
    };
    Entry {
        format_name: name,
        format_id: id,
        category,
        type_label: type_label.into(),
        encoding: encoding.map(|s| s.to_string()),
        description,
        content: Some(Content::Text(content)),
        width: None,
        height: None,
        size: Some(raw_len),
        preview: Some(preview),
    }
}

fn empty_entry(
    name: String,
    id: u32,
    category: &str,
    type_label: &str,
    description: Option<String>,
) -> Entry {
    Entry {
        format_name: name,
        format_id: id,
        category: category.into(),
        type_label: type_label.into(),
        encoding: None,
        description,
        content: None,
        width: None,
        height: None,
        size: None,
        preview: Some("(空)".into()),
    }
}

fn binary_entry(
    name: String,
    id: u32,
    type_label: &str,
    description: Option<String>,
    size: usize,
    preview: &str,
) -> Entry {
    Entry {
        format_name: name,
        format_id: id,
        category: "binary".into(),
        type_label: type_label.into(),
        encoding: None,
        description,
        content: None,
        width: None,
        height: None,
        size: Some(size),
        preview: Some(preview.into()),
    }
}

fn image_entry(
    name: String,
    id: u32,
    type_label: &str,
    description: Option<String>,
    width: Option<i32>,
    height: Option<i32>,
    size: usize,
    preview: String,
) -> Entry {
    Entry {
        format_name: name,
        format_id: id,
        category: "image".into(),
        type_label: type_label.into(),
        encoding: None,
        description,
        content: None,
        width,
        height,
        size: Some(size),
        preview: Some(preview),
    }
}

fn handle_bitmap(
    h_data: HANDLE,
    name: String,
    id: u32,
    description: Option<String>,
) -> windows::core::Result<Entry> {
    let handle_value = h_data.0 as usize;
    let handle_hex = format!("{:#018X}", handle_value);

    Ok(Entry {
        format_name: name,
        format_id: id,
        category: "binary".into(),
        type_label: "CF_BITMAP (GDI 位图句柄)".into(),
        encoding: None,
        description,
        content: Some(Content::Text(handle_hex.clone())),
        width: None,
        height: None,
        size: Some(std::mem::size_of::<HANDLE>()),
        preview: Some(handle_hex),
    })
}

fn handle_dib(
    data: &[u8],
    name: String,
    id: u32,
    description: Option<String>,
) -> windows::core::Result<Entry> {
    if data.len() < 40 {
        return Ok(binary_entry(name, id, "DIB (设备无关位图)", description, data.len(), "(数据不足)"));
    }
    let (width, height, bit_count) = parse_dib_header(data);
    Ok(image_entry(
        name, id, "DIB (设备无关位图)", description,
        Some(width), Some(height), data.len(),
        format!("{} x {} px, {}-bit", width, height, bit_count),
    ))
}

fn handle_dib_v5(
    data: &[u8],
    name: String,
    id: u32,
    description: Option<String>,
) -> windows::core::Result<Entry> {
    if data.len() < 124 {
        return Ok(binary_entry(name, id, "DIBv5 (设备无关位图 v5)", description, data.len(), "(数据不足)"));
    }
    let (width, height, bit_count) = parse_dib_header(data);
    Ok(image_entry(
        name, id, "DIBv5 (设备无关位图 v5)", description,
        Some(width), Some(height), data.len(),
        format!("{} x {} px, {}-bit", width, height, bit_count),
    ))
}

fn parse_dib_header(data: &[u8]) -> (i32, i32, u16) {
    let width = i32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    let height = i32::from_le_bytes([data[8], data[9], data[10], data[11]]).abs();
    let bit_count = u16::from_le_bytes([data[14], data[15]]);
    (width, height, bit_count)
}

fn handle_emf(name: String, id: u32, description: Option<String>) -> windows::core::Result<Entry> {
    Ok(Entry {
        format_name: name,
        format_id: id,
        category: "image".into(),
        type_label: "增强型图元文件 (EMF)".into(),
        encoding: None,
        description,
        content: None,
        width: None,
        height: None,
        size: None,
        preview: Some("(EMF 数据)".into()),
    })
}

unsafe fn handle_file_list(
    h_data: HANDLE,
    name: String,
    id: u32,
    description: Option<String>,
) -> windows::core::Result<Entry> {
    let hdrop = HDROP(h_data.0);
    let file_count = DragQueryFileW(hdrop, 0xFFFFFFFF, None);
    let mut files = Vec::new();

    for i in 0..file_count {
        let len = DragQueryFileW(hdrop, i, None);
        let mut buffer = vec![0u16; (len + 1) as usize];
        DragQueryFileW(hdrop, i, Some(&mut buffer[..len as usize + 1]));
        let file = String::from_utf16_lossy(&buffer[..len as usize]);
        files.push(file);
    }

    let preview = if files.is_empty() {
        "0 个文件".into()
    } else {
        format!("{} 个文件: {}", files.len(), files.join(", "))
    };

    Ok(Entry {
        format_name: name,
        format_id: id,
        category: "files".into(),
        type_label: "文件列表 (HDROP)".into(),
        encoding: None,
        description,
        content: Some(Content::Files(files)),
        width: None,
        height: None,
        size: None,
        preview: Some(preview),
    })
}

pub fn get_format_name(id: u32) -> String {
    match id {
        1 => "CF_TEXT".into(),
        2 => "CF_BITMAP".into(),
        3 => "CF_METAFILEPICT".into(),
        4 => "CF_SYLK".into(),
        5 => "CF_DIF".into(),
        6 => "CF_TIFF".into(),
        7 => "CF_OEMTEXT".into(),
        8 => "CF_DIB".into(),
        9 => "CF_PALETTE".into(),
        10 => "CF_PENDATA".into(),
        11 => "CF_RIFF".into(),
        12 => "CF_WAVE".into(),
        13 => "CF_UNICODETEXT".into(),
        14 => "CF_ENHMETAFILE".into(),
        15 => "CF_HDROP".into(),
        16 => "CF_LOCALE".into(),
        17 => "CF_DIBV5".into(),
        _ => {
            if id >= 0xC000 {
                let mut buf = [0u16; 256];
                let len = unsafe { GetClipboardFormatNameW(id, &mut buf) };
                if len > 0 {
                    String::from_utf16_lossy(&buf[..len as usize])
                } else {
                    format!("Registered#{}", id)
                }
            } else {
                format!("Unknown#{}", id)
            }
        }
    }
}

fn get_format_description(id: u32) -> Option<String> {
    let desc = match id {
        1 => "ANSI 编码文本，CR-LF 换行，null 结尾",
        2 => "设备相关位图 (HBITMAP)",
        3 => "旧式 GDI 图元文件",
        4 => "Microsoft 符号链接格式，ASCII 文本，用于 Excel 数据交换",
        5 => "数据交换格式，ASCII 文本，用于电子表格数据",
        6 => "TIFF 图片格式",
        7 => "OEM 编码文本，CR-LF 换行",
        8 => "设备无关位图 (DIB)",
        9 => "颜色调色板",
        10 => "手写笔输入数据",
        11 => "RIFF 音频数据",
        12 => "WAVE 音频格式",
        13 => "UTF-16LE 编码文本",
        14 => "增强型图元文件 (EMF)",
        15 => "拖放文件列表 (HDROP)",
        16 => "区域设置标识符 (LCID)",
        17 => "设备无关位图 v5 (DIBv5)",
        _ => return None,
    };
    Some(desc.to_string())
}
