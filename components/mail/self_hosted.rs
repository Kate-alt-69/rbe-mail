use rbe_sdk::RbeSdk;
use crate::{http, json};
use crate::types::{error_codes, validate_domain, MailError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MxRecord { pub preference: u16, pub exchange: String }
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
    let granted = |name: &str| sdk.host().granted(name).unwrap_or(false);
    SelfHostedCapabilities {
        dns: granted("net:dns"), outbound_tcp: granted("net:tcp"), tls_upgrade: granted("net:tls"),
        inbound_listener: granted("net:tcp-listen"), persistent_storage: granted("storage"),
        managed_crypto: granted("crypto"), package_service: granted("service:package"),
    }
}

pub fn resolve_mx(sdk: RbeSdk<'_>, domain: &str) -> Result<Vec<MxRecord>, MailError> {
    validate_domain(domain)?; http::require_capability(sdk, "net:dns")?;
    let payload = json::string(domain);
    let reply = sdk.net().dns().call("mx", payload.as_bytes())
        .map_err(|error| MailError::network(format!("RBE net:dns MX lookup failed: {error}")))?;
    let encoded = std::str::from_utf8(&reply.payload).map_err(|_| MailError::external_response("RBE net:dns returned non-UTF8 metadata"))?;
    let mut records = Vec::new();
    for object in json::split_array_objects(encoded, "records") {
        let preference = json::find_u16_field(object, "preference").ok_or_else(|| MailError::external_response("RBE MX response is missing preference"))?;
        let exchange = json::find_string_field(object, "exchange").ok_or_else(|| MailError::external_response("RBE MX response is missing exchange"))?;
        records.push(MxRecord { preference, exchange });
    }
    if records.is_empty() { return Err(MailError::external_response("RBE MX response contained no records")); }
    records.sort_by_key(|record| record.preference); Ok(records)
}

pub fn require_secure_outbound(sdk: RbeSdk<'_>) -> Result<(), MailError> {
    let current = capabilities(sdk);
    if !current.dns { return Err(MailError::capability("net:dns")); }
    if !current.outbound_tcp { return Err(MailError::capability("net:tcp")); }
    if !current.tls_upgrade {
        return Err(MailError::unsupported(error_codes::SMTP_TLS_UNAVAILABLE,
            "self-hosted SMTP needs RBE TLS/STARTTLS authority before direct delivery can start"));
    }
    Ok(())
}

pub fn require_server_mode(sdk: RbeSdk<'_>) -> Result<(), MailError> {
    require_secure_outbound(sdk)?;
    let current = capabilities(sdk);
    if !current.inbound_listener {
        return Err(MailError::unsupported(error_codes::SMTP_LISTENER_UNAVAILABLE,
            "RBE has no package SMTP listen/accept authority granted"));
    }
    if !current.persistent_storage { return Err(MailError::capability("storage")); }
    if !current.managed_crypto { return Err(MailError::capability("crypto")); }
    if !current.package_service {
        return Err(MailError::unsupported(error_codes::PACKAGE_SERVICE_UNAVAILABLE,
            "RBE has no package-owned mail.service activation authority granted"));
    }
    Ok(())
}
