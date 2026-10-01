use rbe_sdk::{HostBridge, RbeSdk};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MxRecord {
    pub preference: u16,
    pub exchange: String,
}

fn objects(input: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'{' {
            let start = i;
            let mut depth = 1usize;
            let mut quoted = false;
            let mut escaped = false;
            i += 1;
            while i < bytes.len() && depth > 0 {
                let byte = bytes[i];
                if quoted {
                    if escaped { escaped = false; }
                    else if byte == b'\\' { escaped = true; }
                    else if byte == b'"' { quoted = false; }
                } else if byte == b'"' { quoted = true; }
                else if byte == b'{' { depth += 1; }
                else if byte == b'}' {
                    depth = depth.saturating_sub(1);
                    if depth == 0 { out.push(&input[start..=i]); }
                }
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    out
}

fn u16_field(input: &str, name: &str) -> Option<u16> {
    let needle = format!("\"{name}\"");
    let tail = input.get(input.find(&needle)? + needle.len()..)?;
    let tail = tail.get(tail.find(':')? + 1..)?.trim_start();
    let end = tail.find(|c: char| !c.is_ascii_digit()).unwrap_or(tail.len());
    tail.get(..end)?.parse().ok()
}

fn string_field(input: &str, name: &str) -> Option<String> {
    let needle = format!("\"{name}\"");
    let tail = input.get(input.find(&needle)? + needle.len()..)?;
    let mut chars = tail.get(tail.find(':')? + 1..)?.trim_start().chars();
    if chars.next()? != '"' { return None; }
    let mut out = String::new();
    let mut escaped = false;
    for ch in chars {
        if escaped {
            match ch {
                '"' => out.push('"'), '\\' => out.push('\\'), '/' => out.push('/'),
                'b' => out.push('\u{08}'), 'f' => out.push('\u{0c}'), 'n' => out.push('\n'),
                'r' => out.push('\r'), 't' => out.push('\t'),
                _ => return None,
            }
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            return Some(out);
        } else {
            out.push(ch);
        }
    }
    None
}

pub fn mx(host: &dyn HostBridge, domain: &str) -> Result<Vec<MxRecord>, String> {
    if domain.trim().is_empty() || domain.contains('/') || domain.chars().any(char::is_control) {
        return Err("MAIL1015: invalid DNS domain; use a fully-qualified public domain name".into());
    }

    let sdk = RbeSdk::new(host);
    match sdk.host().granted("net:dns") {
        Some(true) => {}
        Some(false) => return Err("MAIL2005: RBE net:dns is not granted; approve net:dns for package mail and reactivate the package session".into()),
        None => return Err("MAIL2006: RBE Library Host session is unavailable; execute dns from mail through an accepted package session".into()),
    }

    let payload = format!("\"{}\"", domain.replace('\\', "\\\\").replace('"', "\\\""));
    let reply = sdk.net().dns().call("mx", payload.as_bytes())
        .map_err(|error| format!("MAIL4013: RBE MX lookup failed: {error}; check public DNS and retry transient resolver failures"))?;
    let text = std::str::from_utf8(&reply.payload)
        .map_err(|_| "MAIL8002: RBE DNS response was not UTF-8; preserve the response and report the RBE/package versions".to_string())?;

    let mut result = Vec::new();
    for object in objects(text) {
        let preference = u16_field(object, "preference")
            .ok_or_else(|| "MAIL8002: RBE MX record is missing preference".to_string())?;
        let exchange = string_field(object, "exchange")
            .ok_or_else(|| "MAIL8002: RBE MX record is missing exchange".to_string())?;
        result.push(MxRecord { preference, exchange });
    }
    if result.is_empty() {
        return Err("MAIL4014: no MX records were returned; for SMTP delivery use self-hosted from mail so RFC 5321 A/AAAA fallback is applied".into());
    }
    result.sort_by_key(|record| record.preference);
    Ok(result)
}
