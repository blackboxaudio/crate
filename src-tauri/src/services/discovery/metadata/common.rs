/// Normalize a date string to `YYYY-MM-DD` format.
///
/// Handles:
/// - Already ISO format (`2024-06-19...`) → returns first 10 chars
/// - Bandcamp's `"DD Mon YYYY HH:MM:SS GMT"` format → converts to ISO
/// - Falls back to returning the string as-is
pub(super) fn normalize_date(date_str: &str) -> String {
    let trimmed = date_str.trim();

    // Already ISO format (starts with YYYY-MM-DD)
    if trimmed.len() >= 10 {
        let bytes = trimmed.as_bytes();
        if bytes[4] == b'-' && bytes[7] == b'-' && bytes[..4].iter().all(|b| b.is_ascii_digit()) {
            return trimmed[..10].to_string();
        }
    }

    // Bandcamp format: "19 Jun 2024 00:00:00 GMT"
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.len() >= 3 {
        if let (Ok(day), Some(month_num), Ok(year)) = (
            parts[0].parse::<u32>(),
            month_abbrev_to_num(parts[1]),
            parts[2].parse::<u32>(),
        ) {
            if (1..=31).contains(&day) && (1000..=9999).contains(&year) {
                return format!("{year:04}-{month_num:02}-{day:02}");
            }
        }
    }

    trimmed.to_string()
}

fn month_abbrev_to_num(s: &str) -> Option<u32> {
    match s.to_ascii_lowercase().as_str() {
        "jan" => Some(1),
        "feb" => Some(2),
        "mar" => Some(3),
        "apr" => Some(4),
        "may" => Some(5),
        "jun" => Some(6),
        "jul" => Some(7),
        "aug" => Some(8),
        "sep" => Some(9),
        "oct" => Some(10),
        "nov" => Some(11),
        "dec" => Some(12),
        _ => None,
    }
}

/// Parse ISO 8601 duration (e.g., "PT3M45S" or "P00H03M45S") to milliseconds.
pub(super) fn parse_iso_duration(s: &str) -> Option<i64> {
    // Try standard "PT..." first, then fall back to "P..." (Bandcamp uses P00H06M12S)
    let s = s.strip_prefix("PT").or_else(|| s.strip_prefix("P"))?;
    let mut total_ms: i64 = 0;
    let mut num_buf = String::new();

    for ch in s.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            num_buf.push(ch);
        } else {
            let val: f64 = num_buf.parse().ok()?;
            num_buf.clear();
            match ch {
                'H' => total_ms += (val * 3_600_000.0) as i64,
                'M' => total_ms += (val * 60_000.0) as i64,
                'S' => total_ms += (val * 1_000.0) as i64,
                _ => {}
            }
        }
    }

    if total_ms > 0 {
        Some(total_ms)
    } else {
        None
    }
}

/// Decode common HTML entities in a string (e.g. `&#39;` → `'`).
pub(super) fn decode_html_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }

    let mut result = s
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'");

    // Decode numeric character references: &#NNN; and &#xHH;
    while let Some(start) = result.find("&#") {
        let rest = &result[start + 2..];
        if let Some(end) = rest.find(';') {
            let code_str = &rest[..end];
            let decoded = if let Some(hex) = code_str.strip_prefix('x') {
                u32::from_str_radix(hex, 16).ok()
            } else {
                code_str.parse::<u32>().ok()
            };
            if let Some(ch) = decoded.and_then(char::from_u32) {
                let entity = &result[start..start + 3 + end];
                result = result.replacen(entity, &ch.to_string(), 1);
                continue;
            }
        }
        break;
    }

    result
}

/// Extract content from an OpenGraph meta tag.
pub(super) fn extract_meta_content(html: &str, property: &str) -> Option<String> {
    // Match both property="..." and name="..." patterns
    for attr in ["property", "name"] {
        let pattern = format!("{attr}=\"{property}\"");
        if let Some(pos) = html.find(&pattern) {
            // Look for content="..." nearby (within the same tag)
            let tag_start = html[..pos].rfind('<')?;
            let tag_end = html[pos..].find('>')? + pos;
            let tag = &html[tag_start..=tag_end];

            if let Some(content_start) = tag.find("content=\"") {
                let value_start = content_start + "content=\"".len();
                if let Some(value_end) = tag[value_start..].find('"') {
                    let value = &tag[value_start..value_start + value_end];
                    if !value.is_empty() {
                        return Some(decode_html_entities(value));
                    }
                }
            }
        }
    }
    None
}
