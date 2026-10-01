use rbe_sdk::RbeSdk;
use crate::{http, json};
use crate::types::{error_codes, validate_domain, MailError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MxRecord { pub preference: u16, pub exchange: String, pub implicit: bool }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfHostedCapabilities {
    pub dns: bool, pub outbound_tcp: bool, pub tls_upgrade: bool, pub inbound_listener: bool,
    pub persistent_storage: bool, pub managed_crypto: bool, pub package_service: bool,
}
impl SelfHostedCapabilities {
    pub fn ready_for_outbound_smtp(&self) -> bool { self.dns && self.outbound_tcp && self.tls_upgrade }
    pub fn ready_for_mail_server(&self) -> bool {
        self.ready_for_outbound_smtp() && self.inbound_listener && self.persistent_storage && self.managed_crypto && self.package_service
    }
}

pub fn capabilities(sdk: RbeSdk<'_>) -> SelfHostedCapabilities {
    let granted = |name: &str| sdk.host().granted(name) == Some(true);
    SelfHostedCapabilities {
        dns: granted("net:dns"), outbound_tcp: granted("net:tcp"), tls_upgrade: granted("net:tls"),
        inbound_listener: granted("net:tcp-listen"), persistent_storage: granted("storage"),
        managed_crypto: granted("crypto"), package_service: granted("service:package"),
    }
}

pub fn resolve_mx(sdk: RbeSdk<'_>, domain: &str) -> Result<Vec<MxRecord>, MailError> {
    validate_domain(domain)?;
    http::require_capability(sdk, "net:dns")?;
    let domain = domain.trim_end_matches('.').to_ascii_lowercase();
    let payload = json::string(&domain);
    let reply = match sdk.net().dns().call("mx", payload.as_bytes()) {
        Ok(reply) => reply,
        Err(error) => {
            let message = error.to_string();
            let lower = message.to_ascii_lowercase();
            if lower.contains("no mx records") || lower.contains("no record found") || lower.contains("no records found") {
                return implicit_mx_fallback(sdk, &domain, &payload);
            }
            return Err(MailError::network(format!("RBE net:dns MX lookup failed for {domain}: {message}")));
        }
    };
    let encoded = std::str::from_utf8(&reply.payload).map_err(|_| MailError::external_response("MAIL8002 RBE net:dns returned non-UTF8 MX metadata"))?;
    let mut records = Vec::new();
    for object in json::split_array_objects(encoded, "records") {
        let preference = json::find_u16_field(object, "preference").ok_or_else(|| MailError::external_response("MAIL8002 RBE MX response is missing preference"))?;
        let exchange = json::find_string_field(object, "exchange").ok_or_else(|| MailError::external_response("MAIL8002 RBE MX response is missing exchange"))?;
        records.push(MxRecord { preference, exchange, implicit: false });
    }
    if records.is_empty() { return Err(MailError::external_response("MAIL8002 RBE MX response succeeded but contained no records")); }
    if records.len() == 1 && records[0].preference == 0 && records[0].exchange == "." {
        return Err(MailError::unsupported("MAIL4015", format!("recipient domain {domain} publishes null MX and explicitly does not accept email")));
    }
    if records.iter().any(|record| record.exchange == ".") {
        return Err(MailError::external_response("MAIL8002 MX set illegally mixes null MX with ordinary mail exchangers"));
    }
    records.sort_by_key(|record| record.preference);
    Ok(records)
}

fn implicit_mx_fallback(sdk: RbeSdk<'_>, domain: &str, payload: &str) -> Result<Vec<MxRecord>, MailError> {
    match sdk.net().dns().call("ip", payload.as_bytes()) {
        Ok(_) => Ok(vec![MxRecord { preference: 0, exchange: domain.to_string(), implicit: true }]),
        Err(error) => {
            let message = error.to_string();
            if message.to_ascii_lowercase().contains("no addresses") {
                Err(MailError::unsupported("MAIL4014", format!("recipient domain {domain} has neither MX nor usable A/AAAA records")))
            } else {
                Err(MailError::network(format!("MX was absent and A/AAAA fallback lookup failed for {domain}: {message}")))
            }
        }
    }
}

pub fn require_secure_outbound(sdk: RbeSdk<'_>) -> Result<(), MailError> {
    http::require_capability(sdk, "net:dns")?;
    if sdk.host().granted("net:tcp") != Some(true) {
        return Err(MailError::unsupported("MAIL2003", "RBE net:tcp is not granted/implemented for this package session"));
    }
    if sdk.host().granted("net:tls") != Some(true) {
        return Err(MailError::unsupported(error_codes::SMTP_TLS_UNAVAILABLE,
            "self-hosted SMTP needs RBE TLS/STARTTLS authority; plaintext downgrade is forbidden"));
    }
    Ok(())
}

pub fn require_server_mode(sdk: RbeSdk<'_>) -> Result<(), MailError> {
    require_secure_outbound(sdk)?;
    if sdk.host().granted("net:tcp-listen") != Some(true) {
        return Err(MailError::unsupported(error_codes::SMTP_LISTENER_UNAVAILABLE,
            "RBE has no package SMTP listen/accept authority granted"));
    }
    http::require_capability(sdk, "storage")?;
    http::require_capability(sdk, "crypto")?;
    if sdk.host().granted("service:package") != Some(true) {
        return Err(MailError::unsupported(error_codes::PACKAGE_SERVICE_UNAVAILABLE,
            "RBE has no package-owned mail.service activation authority granted"));
    }
    Ok(())
}
