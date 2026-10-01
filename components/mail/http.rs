use rbe_sdk::RbeSdk;
use crate::json;
use crate::types::{error_codes, MailError, MailErrorKind, ProviderKind};

const RBE_HTTP_BODY_LIMIT: usize = 1024 * 1024;

pub struct HttpResponse { pub status: u16, pub body: String }

pub fn post_json(sdk: RbeSdk<'_>, provider: ProviderKind, url: &str, headers: &[(&str, String)], body: String) -> Result<HttpResponse, MailError> {
    require_capability(sdk, "net:http")?;
    if body.len() > RBE_HTTP_BODY_LIMIT {
        return Err(MailError::new(error_codes::RBE_LIMIT, MailErrorKind::RbeLimit, false,
            format!("RBE net:http request body is {} bytes; maximum is {} bytes", body.len(), RBE_HTTP_BODY_LIMIT)));
    }
    let header_json = encode_headers(headers, Some("application/json"))?;
    let envelope = format!("[{{\"method\":\"POST\",\"url\":{},\"headers\":{},\"body\":{}}}]", json::string(url), header_json, body);
    let reply = sdk.net().http().call("request", envelope.as_bytes()).map_err(map_host_http_error)?;
    parse_response(provider, &reply.payload)
}

pub fn post_raw(sdk: RbeSdk<'_>, provider: ProviderKind, url: &str, content_type: &str, headers: &[(&str, String)], body: String) -> Result<HttpResponse, MailError> {
    require_capability(sdk, "net:http")?;
    if body.len() > RBE_HTTP_BODY_LIMIT {
        return Err(MailError::new(error_codes::RBE_LIMIT, MailErrorKind::RbeLimit, false,
            format!("RBE net:http request body is {} bytes; maximum is {} bytes", body.len(), RBE_HTTP_BODY_LIMIT)));
    }
    if content_type.trim().is_empty() || content_type.chars().any(char::is_control) {
        return Err(MailError::invalid_config("HTTP Content-Type is empty or contains control characters"));
    }
    let header_json = encode_headers(headers, Some(content_type))?;
    let envelope = format!("[{{\"method\":\"POST\",\"url\":{},\"headers\":{},\"body\":{}}}]", json::string(url), header_json, json::string(&body));
    let reply = sdk.net().http().call("request", envelope.as_bytes()).map_err(map_host_http_error)?;
    parse_response(provider, &reply.payload)
}

pub fn require_capability(sdk: RbeSdk<'_>, capability: &'static str) -> Result<(), MailError> {
    match sdk.host().granted(capability) {
        Some(true) => Ok(()),
        Some(false) | None => Err(MailError::capability(capability)),
    }
}

fn encode_headers(headers: &[(&str, String)], forced_content_type: Option<&str>) -> Result<String, MailError> {
    let mut header_json = String::from("{");
    let mut emitted = 0usize;
    for (name, value) in headers {
        if name.eq_ignore_ascii_case("content-type") && forced_content_type.is_some() { continue; }
        validate_header(name, value)?;
        if emitted > 0 { header_json.push(','); }
        header_json.push_str(&json::string(name));
        header_json.push(':');
        header_json.push_str(&json::string(value));
        emitted += 1;
    }
    if let Some(content_type) = forced_content_type {
        if emitted > 0 { header_json.push(','); }
        header_json.push_str("\"Content-Type\":");
        header_json.push_str(&json::string(content_type));
    }
    header_json.push('}');
    Ok(header_json)
}

fn validate_header(name: &str, value: &str) -> Result<(), MailError> {
    if name.is_empty() || !name.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'!' | b'#' | b'$' | b'%' | b'&' | b'\'' | b'*' | b'+' | b'-' | b'.' | b'^' | b'_' | b'`' | b'|' | b'~')) {
        return Err(MailError::invalid_config(format!("HTTP header name {name:?} is invalid")));
    }
    if value.contains('\r') || value.contains('\n') || value.chars().any(|ch| ch.is_control() && ch != '\t') {
        return Err(MailError::invalid_config(format!("HTTP header {name:?} contains forbidden control characters")));
    }
    Ok(())
}

fn map_host_http_error(error: impl std::fmt::Display) -> MailError {
    let message = error.to_string();
    let lower = message.to_ascii_lowercase();
    if lower.contains("exceeds") || lower.contains("too many headers") || lower.contains("maximum") || lower.contains("capability envelope") {
        MailError::new(error_codes::RBE_LIMIT, MailErrorKind::RbeLimit, false,
            format!("RBE net:http rejected a broker limit: {message}"))
    } else {
        MailError::network(format!("RBE net:http failed before a provider response was accepted: {message}"))
    }
}

fn parse_response(provider: ProviderKind, payload: &[u8]) -> Result<HttpResponse, MailError> {
    let encoded = std::str::from_utf8(payload).map_err(|_| MailError::external_response("RBE net:http returned non-UTF8 response metadata"))?;
    let status = json::find_u16_field(encoded, "status").ok_or_else(|| MailError::external_response("RBE net:http response is missing status"))?;
    let ok = json::find_bool_field(encoded, "ok").unwrap_or((200..300).contains(&status));
    let body = json::find_string_field(encoded, "body").unwrap_or_default();
    if !ok { return Err(MailError::provider_status(provider, status)); }
    Ok(HttpResponse { status, body })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn forced_content_type_does_not_duplicate_header() {
        let encoded = encode_headers(&[("Content-Type", "text/plain".into()), ("X-Test", "ok".into())], Some("application/json")).unwrap();
        assert_eq!(encoded.matches("Content-Type").count(), 1);
    }
    #[test]
    fn rejects_header_injection() {
        assert!(validate_header("X-Test", "ok\r\nInjected: yes").is_err());
    }
}
