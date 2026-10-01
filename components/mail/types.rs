use std::fmt;

pub mod error_codes {
    pub const INVALID_ADDRESS: &str = "MAIL1001";
    pub const INVALID_MESSAGE: &str = "MAIL1002";
    pub const INVALID_DOMAIN: &str = "MAIL1003";
    pub const INVALID_CONFIG: &str = "MAIL1004";
    pub const CAPABILITY_MISSING: &str = "MAIL2001";
    pub const SERVER_PREREQUISITE_MISSING: &str = "MAIL2002";
    pub const PROVIDER_AUTH: &str = "MAIL4001";
    pub const PROVIDER_RATE_LIMIT: &str = "MAIL4002";
    pub const PROVIDER_QUOTA: &str = "MAIL4003";
    pub const PROVIDER_REJECTED: &str = "MAIL4004";
    pub const PROVIDER_UNAVAILABLE: &str = "MAIL4005";
    pub const PROVIDER_TIMEOUT: &str = "MAIL4006";
    pub const NETWORK: &str = "MAIL4007";
    pub const RBE_LIMIT: &str = "MAIL4008";
    pub const RECIPIENT_REJECTED: &str = "MAIL4009";
    pub const SMTP_TLS_UNAVAILABLE: &str = "MAIL4010";
    pub const SMTP_LISTENER_UNAVAILABLE: &str = "MAIL4011";
    pub const PACKAGE_SERVICE_UNAVAILABLE: &str = "MAIL4012";
    pub const MALFORMED_EXTERNAL_RESPONSE: &str = "MAIL8001";
    pub const INTERNAL_INVARIANT: &str = "MAIL9001";
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind { Resend, SendGrid, Brevo, Postmark, Mailgun, SendPulse, Ses }
impl ProviderKind {
    pub const fn as_str(self) -> &'static str { match self {
        Self::Resend => "resend", Self::SendGrid => "sendgrid", Self::Brevo => "brevo",
        Self::Postmark => "postmark", Self::Mailgun => "mailgun", Self::SendPulse => "sendpulse", Self::Ses => "ses",
    }}
}

#[derive(Clone, PartialEq, Eq)]
pub struct Address { pub email: String, pub name: Option<String> }
impl Address {
    pub fn new(email: impl Into<String>) -> Self { Self { email: email.into(), name: None } }
    pub fn named(name: impl Into<String>, email: impl Into<String>) -> Self { Self { email: email.into(), name: Some(name.into()) } }
    pub(crate) fn validate(&self) -> Result<(), MailError> {
        let email = self.email.trim();
        let mut parts = email.split('@');
        let local = parts.next().unwrap_or_default();
        let domain = parts.next().unwrap_or_default();
        if local.is_empty() || domain.is_empty() || parts.next().is_some() || domain.starts_with('.')
            || domain.ends_with('.') || !domain.contains('.') || email.chars().any(char::is_control) {
            return Err(MailError::new(error_codes::INVALID_ADDRESS, MailErrorKind::InvalidRequest, false, "email address is invalid"));
        }
        if self.name.as_ref().is_some_and(|name| name.chars().any(char::is_control)) {
            return Err(MailError::new(error_codes::INVALID_ADDRESS, MailErrorKind::InvalidRequest, false, "email display name contains control characters"));
        }
        Ok(())
    }
    pub(crate) fn display(&self) -> String {
        match self.name.as_deref().map(str::trim).filter(|name| !name.is_empty()) {
            Some(name) => format!("{name} <{}>", self.email), None => self.email.clone(),
        }
    }
}
impl fmt::Debug for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.debug_struct("Address").field("email", &self.email).field("name", &self.name).finish() }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Message {
    pub subject: String, pub text: Option<String>, pub html: Option<String>,
    pub cc: Vec<Address>, pub bcc: Vec<Address>, pub reply_to: Option<Address>,
}
impl Message {
    pub fn new(subject: impl Into<String>) -> Self { Self { subject: subject.into(), ..Self::default() } }
    pub fn text(mut self, body: impl Into<String>) -> Self { self.text = Some(body.into()); self }
    pub fn html(mut self, body: impl Into<String>) -> Self { self.html = Some(body.into()); self }
    pub fn cc(mut self, address: Address) -> Self { self.cc.push(address); self }
    pub fn bcc(mut self, address: Address) -> Self { self.bcc.push(address); self }
    pub fn reply_to(mut self, address: Address) -> Self { self.reply_to = Some(address); self }
    pub(crate) fn validate(&self) -> Result<(), MailError> {
        if self.subject.trim().is_empty() { return Err(MailError::invalid_message("email subject cannot be empty")); }
        if self.subject.chars().any(char::is_control) { return Err(MailError::invalid_message("email subject contains control characters")); }
        if self.text.as_deref().is_none_or(str::is_empty) && self.html.as_deref().is_none_or(str::is_empty) {
            return Err(MailError::invalid_message("email needs text or html content"));
        }
        for address in self.cc.iter().chain(&self.bcc) { address.validate()?; }
        if let Some(reply_to) = &self.reply_to { reply_to.validate()?; }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryStatus { Accepted }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendReceipt { pub provider: ProviderKind, pub message_id: Option<String>, pub status: DeliveryStatus }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MailErrorKind {
    CapabilityMissing, InvalidRequest, Authentication, RateLimit, Quota, RecipientRejected,
    ProviderRejected, ProviderUnavailable, Timeout, Network, RbeLimit, Unsupported, ExternalResponse, Internal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MailError {
    pub code: &'static str,
    pub kind: MailErrorKind,
    pub provider: Option<ProviderKind>,
    pub retryable: bool,
    pub status_code: Option<u16>,
    pub message: String,
}
impl MailError {
    pub(crate) fn new(code: &'static str, kind: MailErrorKind, retryable: bool, message: impl Into<String>) -> Self {
        Self { code, kind, provider: None, retryable, status_code: None, message: message.into() }
    }
    pub(crate) fn invalid_message(message: impl Into<String>) -> Self { Self::new(error_codes::INVALID_MESSAGE, MailErrorKind::InvalidRequest, false, message) }
    pub(crate) fn invalid_config(message: impl Into<String>) -> Self { Self::new(error_codes::INVALID_CONFIG, MailErrorKind::InvalidRequest, false, message) }
    pub(crate) fn invalid_domain(message: impl Into<String>) -> Self { Self::new(error_codes::INVALID_DOMAIN, MailErrorKind::InvalidRequest, false, message) }
    pub(crate) fn capability(capability: &'static str) -> Self {
        Self::new(error_codes::CAPABILITY_MISSING, MailErrorKind::CapabilityMissing, false, format!("RBE host capability {capability} is not granted"))
    }
    pub(crate) fn network(message: impl Into<String>) -> Self { Self::new(error_codes::NETWORK, MailErrorKind::Network, true, message) }
    pub(crate) fn external_response(message: impl Into<String>) -> Self { Self::new(error_codes::MALFORMED_EXTERNAL_RESPONSE, MailErrorKind::ExternalResponse, false, message) }
    pub(crate) fn unsupported(code: &'static str, message: impl Into<String>) -> Self { Self::new(code, MailErrorKind::Unsupported, false, message) }
    pub(crate) fn invariant(message: impl Into<String>) -> Self { Self::new(error_codes::INTERNAL_INVARIANT, MailErrorKind::Internal, false, message) }
    pub(crate) fn provider_status(provider: ProviderKind, status: u16) -> Self {
        let (code, kind, retryable, message) = match status {
            401 | 403 => (error_codes::PROVIDER_AUTH, MailErrorKind::Authentication, false, "provider rejected authentication"),
            408 | 504 => (error_codes::PROVIDER_TIMEOUT, MailErrorKind::Timeout, true, "provider request timed out"),
            429 => (error_codes::PROVIDER_RATE_LIMIT, MailErrorKind::RateLimit, true, "provider rate limit exceeded"),
            402 => (error_codes::PROVIDER_QUOTA, MailErrorKind::Quota, false, "provider quota or billing limit was reached"),
            500..=599 => (error_codes::PROVIDER_UNAVAILABLE, MailErrorKind::ProviderUnavailable, true, "provider is temporarily unavailable"),
            _ => (error_codes::PROVIDER_REJECTED, MailErrorKind::ProviderRejected, false, "provider rejected the email request"),
        };
        let mut error = Self::new(code, kind, retryable, message);
        error.provider = Some(provider); error.status_code = Some(status); error
    }
    pub fn help_anchor(&self) -> String { format!("doc/error-codes/mail.md#{}", self.code.to_ascii_lowercase()) }
}
impl fmt::Display for MailError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{} {}", self.code, self.message) }
}
impl std::error::Error for MailError {}

#[derive(Clone)]
pub enum Transport {
    Resend { api_key: String, from: Address }, SendGrid { api_key: String, from: Address },
    Brevo { api_key: String, from: Address },
    Postmark { server_token: String, from: Address, message_stream: Option<String> },
    Mailgun { api_key: String, domain: String, from: Address, region: MailgunRegion },
    SendPulse { access_token: String, from: Address },
    Ses { access_key: String, secret_key: String, session_token: Option<String>, region: String, from: Address },
    SelfHosted { domain: String, from: Address },
}
impl fmt::Debug for Transport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { match self {
        Self::Resend { from, .. } => f.debug_struct("Resend").field("from", from).finish(),
        Self::SendGrid { from, .. } => f.debug_struct("SendGrid").field("from", from).finish(),
        Self::Brevo { from, .. } => f.debug_struct("Brevo").field("from", from).finish(),
        Self::Postmark { from, message_stream, .. } => f.debug_struct("Postmark").field("from", from).field("message_stream", message_stream).finish(),
        Self::Mailgun { domain, from, region, .. } => f.debug_struct("Mailgun").field("domain", domain).field("from", from).field("region", region).finish(),
        Self::SendPulse { from, .. } => f.debug_struct("SendPulse").field("from", from).finish(),
        Self::Ses { region, from, session_token, .. } => f.debug_struct("Ses").field("region", region).field("from", from).field("session_token", &session_token.as_ref().map(|_| "<redacted>")).finish(),
        Self::SelfHosted { domain, from } => f.debug_struct("SelfHosted").field("domain", domain).field("from", from).finish(),
    }}
}
impl Transport {
    pub fn resend(api_key: impl Into<String>, from: Address) -> Self { Self::Resend { api_key: api_key.into(), from } }
    pub fn sendgrid(api_key: impl Into<String>, from: Address) -> Self { Self::SendGrid { api_key: api_key.into(), from } }
    pub fn brevo(api_key: impl Into<String>, from: Address) -> Self { Self::Brevo { api_key: api_key.into(), from } }
    pub fn postmark(server_token: impl Into<String>, from: Address) -> Self { Self::Postmark { server_token: server_token.into(), from, message_stream: None } }
    pub fn postmark_stream(mut self, stream: impl Into<String>) -> Self { if let Self::Postmark { message_stream, .. } = &mut self { *message_stream = Some(stream.into()); } self }
    pub fn mailgun(api_key: impl Into<String>, domain: impl Into<String>, from: Address) -> Self { Self::Mailgun { api_key: api_key.into(), domain: domain.into(), from, region: MailgunRegion::Us } }
    pub fn mailgun_region(mut self, region: MailgunRegion) -> Self { if let Self::Mailgun { region: current, .. } = &mut self { *current = region; } self }
    pub fn sendpulse(access_token: impl Into<String>, from: Address) -> Self { Self::SendPulse { access_token: access_token.into(), from } }
    pub fn ses(access_key: impl Into<String>, secret_key: impl Into<String>, region: impl Into<String>, from: Address) -> Self { Self::Ses { access_key: access_key.into(), secret_key: secret_key.into(), session_token: None, region: region.into(), from } }
    pub fn ses_session_token(mut self, token: impl Into<String>) -> Self { if let Self::Ses { session_token, .. } = &mut self { *session_token = Some(token.into()); } self }
    pub fn self_hosted(domain: impl Into<String>, from: Address) -> Self { Self::SelfHosted { domain: domain.into(), from } }
    pub(crate) fn validate(&self) -> Result<(), MailError> {
        let (secret, from) = match self {
            Self::Resend { api_key, from } | Self::SendGrid { api_key, from } | Self::Brevo { api_key, from } => (Some(api_key.as_str()), from),
            Self::Postmark { server_token, from, .. } => (Some(server_token.as_str()), from),
            Self::Mailgun { api_key, domain, from, .. } => { validate_domain(domain)?; (Some(api_key.as_str()), from) }
            Self::SendPulse { access_token, from } => (Some(access_token.as_str()), from),
            Self::Ses { access_key, secret_key, region, from, .. } => {
                if access_key.trim().is_empty() || secret_key.trim().is_empty() || region.trim().is_empty() { return Err(MailError::invalid_config("AWS SES credentials/region cannot be empty")); }
                (None, from)
            }
            Self::SelfHosted { domain, from } => { validate_domain(domain)?; (None, from) }
        };
        if secret.is_some_and(|secret| secret.trim().is_empty()) { return Err(MailError::invalid_config("provider credential cannot be empty")); }
        from.validate()?; Ok(())
    }
    pub(crate) const fn provider_kind(&self) -> Option<ProviderKind> { match self {
        Self::Resend { .. } => Some(ProviderKind::Resend), Self::SendGrid { .. } => Some(ProviderKind::SendGrid),
        Self::Brevo { .. } => Some(ProviderKind::Brevo), Self::Postmark { .. } => Some(ProviderKind::Postmark),
        Self::Mailgun { .. } => Some(ProviderKind::Mailgun), Self::SendPulse { .. } => Some(ProviderKind::SendPulse), Self::Ses { .. } => Some(ProviderKind::Ses), Self::SelfHosted { .. } => None,
    }}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MailgunRegion { Us, Eu }
pub(crate) fn validate_domain(domain: &str) -> Result<(), MailError> {
    let domain = domain.trim_end_matches('.');
    if domain.is_empty() || domain.len() > 253 || !domain.contains('.') || domain.chars().any(char::is_control) {
        return Err(MailError::invalid_domain("mail domain is invalid"));
    }
    for label in domain.split('.') {
        if label.is_empty() || label.len() > 63 || label.starts_with('-') || label.ends_with('-')
            || !label.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-') {
            return Err(MailError::invalid_domain("mail domain contains an invalid DNS label"));
        }
    }
    Ok(())
}
