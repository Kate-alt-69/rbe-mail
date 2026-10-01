use rbe_sdk::{HostBridge, RbeSdk};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Security { StartTls, TlsWrapper }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmtpProfile {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub security: Security,
}

impl SmtpProfile {
    pub fn new(
        host: impl Into<String>,
        port: u16,
        username: impl Into<String>,
        password: impl Into<String>,
        security: Security,
    ) -> Result<Self, String> {
        let host = host.into();
        let username = username.into();
        let password = password.into();
        if host.trim().is_empty() || port == 0 || host.chars().any(char::is_control) {
            return Err("MAIL1013: invalid SMTP endpoint; provide a non-empty host and port 1..65535".into());
        }
        if username.is_empty() || password.is_empty() {
            return Err("MAIL1014: SMTP credentials cannot be empty; use the provider's SMTP credentials, which may differ from API keys".into());
        }
        Ok(Self { host, port, username, password, security })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capabilities {
    pub session: bool,
    pub tcp: bool,
    pub tls: bool,
}

pub fn capabilities(host: &dyn HostBridge) -> Capabilities {
    let sdk = RbeSdk::new(host);
    let session = sdk.host().session().is_some();
    Capabilities {
        session,
        tcp: sdk.host().granted("net:tcp") == Some(true),
        tls: sdk.host().granted("net:tls") == Some(true),
    }
}

pub fn require_secure_transport(host: &dyn HostBridge) -> Result<(), String> {
    let sdk = RbeSdk::new(host);
    if sdk.host().session().is_none() {
        return Err("MAIL2006: RBE Library Host session is unavailable; run smtp from mail through an accepted package session".into());
    }
    if sdk.host().granted("net:tcp") != Some(true) {
        return Err("MAIL2003: RBE net:tcp is not granted/implemented for this package session; use a provider API until RBE supplies TCP authority".into());
    }
    if sdk.host().granted("net:tls") != Some(true) {
        return Err("MAIL2004: RBE TLS/STARTTLS authority is not granted/implemented; do not downgrade to plaintext SMTP".into());
    }
    Ok(())
}
