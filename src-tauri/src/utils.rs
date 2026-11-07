pub fn calculate_hash(data: &[u8]) -> String {
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut hash = FNV_OFFSET_BASIS;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("{:016x}", hash)
}

pub fn hex_dump(data: &[u8], max_bytes: usize) -> String {
    let count = data.len().min(max_bytes);
    let mut result = String::new();
    for i in (0..count).step_by(8) {
        result.push_str(&format!("{:08X}  ", i));
        let line_len = 8.min(count - i);
        for j in 0..8 {
            if j < line_len {
                result.push_str(&format!("<span class=\"hex\">{:02X}</span> ", data[i + j]));
            } else {
                result.push_str("   ");
            }
        }
        result.push_str(" |");
        for j in 0..line_len {
            let b = data[i + j];
            let ch = if b >= 0x20 && b <= 0x7E { b as char } else { '.' };
            result.push_str(&format!("<span class=\"ascii\">{}</span>", escape_html_char(ch)));
        }
        result.push_str("|\n");
    }
    if data.len() > max_bytes {
        result.push_str(&format!("  ... (还有 {} 字节)\n", data.len() - max_bytes));
    }
    result.trim_end().to_string()
}

fn escape_html_char(c: char) -> String {
    match c {
        '&' => "&amp;".to_string(),
        '<' => "&lt;".to_string(),
        '>' => "&gt;".to_string(),
        '"' => "&quot;".to_string(),
        '\'' => "&#039;".to_string(),
        _ => c.to_string(),
    }
}

pub fn is_ascii(data: &[u8]) -> bool {
    data.iter().all(|&b| b < 0x80)
}

pub fn decode_text_bytes(data: &[u8]) -> (String, &'static str) {
    if data.is_empty() {
        return (String::new(), "unknown");
    }
    let mut detector = chardetng::EncodingDetector::new();
    detector.feed(data, true);
    let encoding = detector.guess(None, true);
    let encoding_name = encoding.name();
    let (text, _, _) = encoding.decode(data);
    (text.trim_end_matches('\0').to_string(), encoding_name)
}

pub fn safe_truncate(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

pub fn is_printable_text(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let check_len = s.len().min(200);
    let printable = s.chars().take(check_len).filter(|&c| {
        c == '\r' || c == '\n' || c == '\t' || (c >= ' ' && c != '\u{FFFE}' && c != '\u{FEFF}')
    }).count();
    printable as f64 / check_len as f64 > 0.85
}

pub fn classify_text(text: &str) -> String {
    let trimmed = text.trim_start();
    let lower = trimmed.to_lowercase();
    if lower.contains("<html") {
        return "html".into();
    }
    if trimmed.starts_with("{\\rtf") || trimmed.starts_with("{\\urtf") {
        return "rtf".into();
    }
    "text".into()
}

pub fn category_order(category: &str) -> u8 {
    match category {
        "html" => 0,
        "rtf" => 1,
        "files" => 2,
        "image" => 3,
        "text" => 4,
        "binary" => 5,
        "empty" => 6,
        "error" => 7,
        _ => 8,
    }
}
