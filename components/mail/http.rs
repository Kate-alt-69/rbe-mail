use rbe_sdk::RbeSdk;
use crate::json;
use crate::types::{MailError, ProviderKind};

pub struct HttpResponse { pub status: u16, pub body: String }

pub fn post_json(sdk: RbeSdk<'_>, provider: ProviderKind, url: &str, headers: &[(&str, String)], body: String) -> Result<HttpResponse, MailError> {
    require_capability(sdk, "net:http")?;
    let mut header_json = String::from("{");
    for (index, (name, value)) in headers.iter().enumerate() {
        if index > 0 { header_json.push(','); }
        header_json.push_str(&json::string(name)); header_json.push(':'); header_json.push_str(&json::string(value));
    }
    if !headers.iter().any(|(name, _)| name.eq_ignore_ascii_case("content-type")) {
        if !headers.is_empty() { header_json.push(','); }
        header_json.push_str("\"Content-Type\":\"application/json\"");
    }
    header_json.push('}');
    let envelope = format!("[{{\"method\":\"POST\",\"url\":{},\"headers\":{},\"body\":{}}}]", json::string(url), header_json, body);
    let reply = sdk.net().http().call("request", envelope.as_bytes())
        .map_err(|error| MailError::network(format!("RBE net:http failed: {error}")))?;
    parse_response(provider, &reply.payload)
}

pub fn post_raw(sdk: RbeSdk<'_>, provider: ProviderKind, url: &str, content_type: &str, headers: &[(&str, String)], body: String) -> Result<HttpResponse, MailError> {
    require_capability(sdk, "net:http")?;
    let mut header_json = String::from("{");
    for (index, (name, value)) in headers.iter().enumerate() {
        if index > 0 { header_json.push(','); }
        header_json.push_str(&json::string(name)); header_json.push(':'); header_json.push_str(&json::string(value));
    }
    if !headers.is_empty() { header_json.push(','); }
    header_json.push_str(&json::string("Content-Type")); header_json.push(':'); header_json.push_str(&json::string(content_type)); header_json.push('}');
    let envelope = format!("[{{\"method\":\"POST\",\"url\":{},\"headers\":{},\"body\":{}}}]", json::string(url), header_json, json::string(&body));
    let reply = sdk.net().http().call("request", envelope.as_bytes())
        .map_err(|error| MailError::network(format!("RBE net:http failed: {error}")))?;
    parse_response(provider, &reply.payload)
}

pub fn require_capability(sdk: RbeSdk<'_>, capability: &'static str) -> Result<(), MailError> {
    if sdk.host().granted(capability) == Some(false) { return Err(MailError::capability(capability)); }
    Ok(())
}

fn parse_response(provider: ProviderKind, payload: &[u8]) -> Result<HttpResponse, MailError> {
    let encoded = std::str::from_utf8(payload).map_err(|_| MailError::external_response("RBE net:http returned non-UTF8 response metadata"))?;
    let status = json::find_u16_field(encoded, "status").ok_or_else(|| MailError::external_response("RBE net:http response is missing status"))?;
    let ok = json::find_bool_field(encoded, "ok").unwrap_or((200..300).contains(&status));
    let body = json::find_string_field(encoded, "body").unwrap_or_default();
    if !ok { return Err(MailError::provider_status(provider, status)); }
    Ok(HttpResponse { status, body })
}
