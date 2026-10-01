pub fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 8);
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            value if value.is_control() => {
                use std::fmt::Write as _;
                let _ = write!(out, "\\u{:04x}", value as u32);
            }
            value => out.push(value),
        }
    }
    out
}

pub fn string(value: &str) -> String { format!("\"{}\"", escape(value)) }

pub fn find_u16_field(input: &str, field: &str) -> Option<u16> {
    let index = find_field_value(input, field)?;
    let bytes = input.as_bytes();
    let mut end = index;
    while end < bytes.len() && bytes[end].is_ascii_digit() { end += 1; }
    (end > index).then(|| input[index..end].parse().ok()).flatten()
}

pub fn find_bool_field(input: &str, field: &str) -> Option<bool> {
    let index = find_field_value(input, field)?;
    input[index..].strip_prefix("true").map(|_| true)
        .or_else(|| input[index..].strip_prefix("false").map(|_| false))
}

pub fn find_string_field(input: &str, field: &str) -> Option<String> {
    let mut index = find_field_value(input, field)?;
    let bytes = input.as_bytes();
    if bytes.get(index) != Some(&b'"') { return None; }
    index += 1;
    let mut out = String::new();
    while index < bytes.len() {
        match bytes[index] {
            b'"' => return Some(out),
            b'\\' => {
                index += 1;
                match *bytes.get(index)? {
                    b'"' => out.push('"'), b'\\' => out.push('\\'), b'/' => out.push('/'),
                    b'b' => out.push('\u{08}'), b'f' => out.push('\u{0c}'), b'n' => out.push('\n'),
                    b'r' => out.push('\r'), b't' => out.push('\t'),
                    b'u' => {
                        let hex = input.get(index + 1..index + 5)?;
                        let value = u16::from_str_radix(hex, 16).ok()?;
                        out.push(char::from_u32(value as u32)?);
                        index += 4;
                    }
                    _ => return None,
                }
            }
            byte if byte < 0x80 => out.push(byte as char),
            _ => {
                let tail = input.get(index..)?;
                let ch = tail.chars().next()?;
                out.push(ch);
                index += ch.len_utf8().saturating_sub(1);
            }
        }
        index += 1;
    }
    None
}

pub fn split_array_objects<'a>(input: &'a str, field: &str) -> Vec<&'a str> {
    let Some(mut index) = find_field_value(input, field) else { return Vec::new(); };
    let bytes = input.as_bytes();
    if bytes.get(index) != Some(&b'[') { return Vec::new(); }
    index += 1;
    let mut output = Vec::new();
    while index < bytes.len() {
        skip_ws(bytes, &mut index);
        if bytes.get(index) == Some(&b']') { break; }
        if bytes.get(index) != Some(&b'{') { break; }
        let start = index;
        let mut depth = 0usize;
        let mut quoted = false;
        let mut escaped = false;
        while index < bytes.len() {
            let byte = bytes[index];
            if quoted {
                if escaped { escaped = false; }
                else if byte == b'\\' { escaped = true; }
                else if byte == b'"' { quoted = false; }
            } else if byte == b'"' { quoted = true; }
            else if byte == b'{' { depth += 1; }
            else if byte == b'}' {
                depth = depth.saturating_sub(1);
                if depth == 0 { output.push(&input[start..=index]); index += 1; break; }
            }
            index += 1;
        }
        skip_ws(bytes, &mut index);
        if bytes.get(index) == Some(&b',') { index += 1; }
    }
    output
}

fn find_field_value(input: &str, field: &str) -> Option<usize> {
    let pattern = format!("\"{}\"", field);
    let bytes = input.as_bytes();
    let mut search = 0usize;
    while search < input.len() {
        let relative = input.get(search..)?.find(&pattern)?;
        let mut index = search + relative + pattern.len();
        skip_ws(bytes, &mut index);
        if bytes.get(index) != Some(&b':') { search = index.saturating_add(1); continue; }
        index += 1; skip_ws(bytes, &mut index); return Some(index);
    }
    None
}

fn skip_ws(bytes: &[u8], index: &mut usize) {
    while bytes.get(*index).is_some_and(|b| b.is_ascii_whitespace()) { *index += 1; }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extracts_escaped_strings() {
        let input = r#"{"status":202,"ok":true,"body":"{\"id\":\"abc\\ndef\"}"}"#;
        assert_eq!(find_u16_field(input, "status"), Some(202));
        assert_eq!(find_bool_field(input, "ok"), Some(true));
        assert_eq!(find_string_field(input, "body").unwrap(), "{\"id\":\"abc\ndef\"}");
    }
}
