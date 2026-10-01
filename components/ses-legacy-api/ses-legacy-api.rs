use rbe_sdk::{HostBridge, RbeSdk};

fn diagnostic_detail(body: &str) -> String {
    let mut clean = body
        .chars()
        .map(|ch| if ch.is_control() && !matches!(ch, '\n' | '\r' | '\t') { '�' } else { ch })
        .collect::<String>();
    if clean.len() > 1024 {
        let mut boundary = 1024;
        while !clean.is_char_boundary(boundary) { boundary -= 1; }
        clean.truncate(boundary);
        clean.push_str("…");
    }
    clean
}

fn map_host_http_error(message: &str) -> String {
    let lower = message.to_ascii_lowercase();
    if lower.contains("exceeds") || lower.contains("too many headers") || lower.contains("maximum") || lower.contains("capability envelope") {
        format!("MAIL4008: RBE net:http rejected a request/response broker limit: {message}; reduce message/header/body size")
    } else {
        format!("MAIL4007: RBE net:http failed before a provider response was accepted: {message}; check RBE network/DNS/TLS reachability")
    }
}

fn provider_http_error(status: u16, body: &str) -> String {
    let (code, reason, fix) = match status {
        401 | 403 => ("MAIL4001", "provider authentication/authorization was rejected", "verify/rotate the credential and sender permissions"),
        402 => ("MAIL4003", "provider quota or billing limit was reached", "fix provider billing/quota before retrying"),
        408 | 504 => ("MAIL4006", "provider request timed out", "delivery can be ambiguous; check provider/idempotency state before resending"),
        429 => ("MAIL4002", "provider rate limit was exceeded", "respect Retry-After/backoff and reduce concurrency"),
        500..=599 => ("MAIL4005", "provider is temporarily unavailable", "retry with backoff or deliberately switch transport"),
        _ => ("MAIL4004", "provider rejected the request", "inspect provider detail and correct request/sender/recipient fields"),
    };
    let detail = diagnostic_detail(body);
    if detail.trim().is_empty() {
        format!("{code}: HTTP {status}: {reason}; fix: {fix}")
    } else {
        format!("{code}: HTTP {status}: {reason}; detail: {detail}; fix: {fix}")
    }
}

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
        return Err("MAIL1005: invalid provider-local API path; fix: pass a provider-local path beginning with one / and no scheme/traversal".into());
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
    match sdk.host().granted("net:http") {
        Some(true) => {}
        Some(false) => return Err("MAIL2001: RBE net:http capability is not granted; approve net:http for package mail and reactivate the package session".into()),
        None => return Err("MAIL2006: RBE Library Host session is unavailable; execute this component through an accepted RBE package session".into()),
    }
    let method = method.to_ascii_uppercase();
    if !matches!(method.as_str(), "GET" | "POST" | "PUT" | "PATCH" | "DELETE") {
        return Err("MAIL1006: unsupported provider HTTP method; fix: use GET/POST/PUT/PATCH/DELETE supported by the provider endpoint".into());
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
        .map_err(|e| map_host_http_error(&e.to_string()))?;
    let encoded = std::str::from_utf8(&reply.payload)
        .map_err(|_| "MAIL8001: RBE net:http returned non-UTF8 metadata; fix: preserve the response context and report the RBE/package versions".to_string())?;
    let status = field_u16(encoded, "status")
        .ok_or_else(|| "MAIL8001: RBE net:http response is missing status; fix: preserve the response context and report the RBE/package versions".to_string())?;
    let ok = field_bool(encoded, "ok").unwrap_or((200..300).contains(&status));
    let body = field_string(encoded, "body").unwrap_or_default();
    if !ok { return Err(provider_http_error(status, &body)); }
    Ok(ApiResponse { status, ok, body })
}


mod sigv4;

fn form(value:&str)->String{
    let mut out=String::new();
    for b in value.bytes(){
        if b.is_ascii_alphanumeric()||matches!(b,b'-'|b'_'|b'.'|b'~'){out.push(b as char)}
        else{use std::fmt::Write as _;let _=write!(out,"%{b:02X}");}
    } out
}
pub struct SesLegacyApi<'a>{
    sdk:RbeSdk<'a>,access:String,secret:String,token:Option<String>,region:String
}
impl<'a> SesLegacyApi<'a>{
    pub fn new(host:&'a dyn HostBridge,access:impl Into<String>,secret:impl Into<String>,region:impl Into<String>)->Result<Self,String>{
        let access=access.into();let secret=secret.into();let region=region.into();
        if access.is_empty()||secret.is_empty(){return Err("MAIL1007: AWS credentials cannot be empty".into());}
        Ok(Self{sdk:RbeSdk::new(host),access,secret,token:None,region})
    }
    pub fn session_token(mut self,t:impl Into<String>)->Self{self.token=Some(t.into());self}
    pub fn send_email(&self,source:&str,to:&str,subject:&str,text:&str)->Result<ApiResponse,String>{
        let body=format!("Action=SendEmail&Version=2010-12-01&Source={}&Destination.ToAddresses.member.1={}&Message.Subject.Data={}&Message.Body.Text.Data={}",
            form(source),form(to),form(subject),form(text));
        let host=format!("email.{}.amazonaws.com",self.region);
        let ct="application/x-www-form-urlencoded; charset=utf-8";
        let mut headers=sigv4::sign(&self.access,&self.secret,self.token.as_deref(),&self.region,&host,"POST","/",ct,body.as_bytes())?;
        headers.push(("Content-Type",ct.into()));
        http(self.sdk,"POST",&format!("https://{host}/"),&headers,Some(&jstr(&body)))
    }
}
