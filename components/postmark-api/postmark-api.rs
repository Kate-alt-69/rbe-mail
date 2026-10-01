use rbe_sdk::{HostBridge, RbeSdk};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiResponse {
    pub status: u16,
    pub ok: bool,
    pub body: String,
}

fn esc(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                use std::fmt::Write as _;
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

fn jstr(value: &str) -> String { format!("\"{}\"", esc(value)) }

fn field_u16(input: &str, field: &str) -> Option<u16> {
    let needle = format!("\"{field}\"");
    let tail = input.get(input.find(&needle)? + needle.len()..)?;
    let tail = tail.get(tail.find(':')? + 1..)?.trim_start();
    let end = tail.find(|c: char| !c.is_ascii_digit()).unwrap_or(tail.len());
    tail.get(..end)?.parse().ok()
}

fn field_bool(input: &str, field: &str) -> Option<bool> {
    let needle = format!("\"{field}\"");
    let tail = input.get(input.find(&needle)? + needle.len()..)?;
    let tail = tail.get(tail.find(':')? + 1..)?.trim_start();
    if tail.starts_with("true") { Some(true) }
    else if tail.starts_with("false") { Some(false) }
    else { None }
}

fn field_string(input: &str, field: &str) -> Option<String> {
    let needle = format!("\"{field}\"");
    let tail = input.get(input.find(&needle)? + needle.len()..)?;
    let mut chars = tail.get(tail.find(':')? + 1..)?.trim_start().chars();
    if chars.next()? != '"' { return None; }
    let mut out = String::new();
    let mut escaped = false;
    for ch in chars {
        if escaped {
            match ch {
                '"' => out.push('"'),
                '\\' => out.push('\\'),
                '/' => out.push('/'),
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                other => out.push(other),
            }
            escaped = false;
        } else {
            match ch {
                '\\' => escaped = true,
                '"' => return Some(out),
                other => out.push(other),
            }
        }
    }
    None
}

fn validate_path(path: &str) -> Result<(), String> {
    if !path.starts_with('/') || path.starts_with("//") || path.contains("://") ||
       path.contains('\\') || path.split('/').any(|part| part == "..") {
        return Err("MAIL1005: invalid provider-local API path".into());
    }
    Ok(())
}

fn http(
    sdk: RbeSdk<'_>,
    method: &str,
    url: &str,
    headers: &[(&str, String)],
    body: Option<&str>,
) -> Result<ApiResponse, String> {
    if sdk.host().granted("net:http") == Some(false) {
        return Err("MAIL2001: RBE net:http capability is not granted".into());
    }
    let method = method.to_ascii_uppercase();
    if !matches!(method.as_str(), "GET" | "POST" | "PUT" | "PATCH" | "DELETE") {
        return Err("MAIL1006: unsupported provider HTTP method".into());
    }

    let mut h = String::from("{");
    for (i, (name, value)) in headers.iter().enumerate() {
        if i > 0 { h.push(','); }
        h.push_str(&jstr(name));
        h.push(':');
        h.push_str(&jstr(value));
    }
    if !headers.iter().any(|(name, _)| name.eq_ignore_ascii_case("content-type")) {
        if !headers.is_empty() { h.push(','); }
        h.push_str("\"Content-Type\":\"application/json\"");
    }
    h.push('}');

    let envelope = format!(
        "[{{\"method\":{},\"url\":{},\"headers\":{},\"body\":{}}}]",
        jstr(&method), jstr(url), h, body.unwrap_or("null")
    );

    let reply = sdk.net().http().call("request", envelope.as_bytes())
        .map_err(|e| format!("MAIL4001: RBE net:http failed: {e}"))?;
    let encoded = std::str::from_utf8(&reply.payload)
        .map_err(|_| "MAIL8001: RBE net:http returned non-UTF8 metadata".to_string())?;
    let status = field_u16(encoded, "status")
        .ok_or_else(|| "MAIL8001: response is missing status".to_string())?;
    let ok = field_bool(encoded, "ok").unwrap_or((200..300).contains(&status));
    let body = field_string(encoded, "body").unwrap_or_default();
    Ok(ApiResponse { status, ok, body })
}

pub struct PostmarkApi<'a> {
    sdk: RbeSdk<'a>,
    credential: String,
    base_url: String,
}

impl<'a> PostmarkApi<'a> {
    pub fn new(host: &'a dyn HostBridge, credential: impl Into<String>) -> Result<Self, String> {
        let credential = credential.into();
        if credential.trim().is_empty() {
            return Err("MAIL1007: provider credential cannot be empty".into());
        }
        Ok(Self {
            sdk: RbeSdk::new(host),
            credential,
            base_url: "https://api.postmarkapp.com".into(),
        })
    }

    pub fn request_json(&self, method: &str, path: &str, body: Option<&str>) -> Result<ApiResponse, String> {
        validate_path(path)?;
        let headers = vec![("X-Postmark-Server-Token", self.credential.clone())];
        http(self.sdk, method, &format!("{}{}", self.base_url, path), &headers, body)
    }


    pub fn send_email(&self, payload: &str) -> Result<ApiResponse, String> {
        self.request_json("POST", "/email", Some(payload))
    }
    pub fn send_batch(&self, payload: &str) -> Result<ApiResponse, String> {
        self.request_json("POST", "/email/batch", Some(payload))
    }

}
