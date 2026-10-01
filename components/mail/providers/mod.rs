mod brevo; mod mailgun; mod postmark; mod resend; mod sendgrid;
mod sendpulse;
mod ses;
mod sigv4;
use rbe_sdk::RbeSdk;
use crate::types::{Address, Message, MailError, SendReceipt, Transport};

pub fn send(sdk: RbeSdk<'_>, transport: &Transport, to: &[Address], message: &Message) -> Result<SendReceipt, MailError> {
    match transport {
        Transport::Resend { api_key, from } => resend::send(sdk, api_key, from, to, message),
        Transport::SendGrid { api_key, from } => sendgrid::send(sdk, api_key, from, to, message),
        Transport::Brevo { api_key, from } => brevo::send(sdk, api_key, from, to, message),
        Transport::Postmark { server_token, from, message_stream } => postmark::send(sdk, server_token, from, message_stream.as_deref(), to, message),
        Transport::Mailgun { api_key, domain, from, region } => mailgun::send(sdk, api_key, domain, from, *region, to, message),
        Transport::SendPulse { access_token, from } => sendpulse::send(sdk, access_token, from, to, message),
        Transport::Ses { access_key, secret_key, session_token, region, from } => ses::send(sdk, access_key, secret_key, session_token.as_deref(), region, from, to, message),
        Transport::SelfHosted { .. } => Err(MailError::invariant("self-hosted transport reached provider dispatcher")),
    }
}

pub(crate) fn address_object(address: &Address) -> String {
    match address.name.as_deref().filter(|name| !name.trim().is_empty()) {
        Some(name) => format!("{{\"email\":{},\"name\":{}}}", crate::json::string(&address.email), crate::json::string(name)),
        None => format!("{{\"email\":{}}}", crate::json::string(&address.email)),
    }
}
pub(crate) fn address_array(addresses: &[Address]) -> String {
    format!("[{}]", addresses.iter().map(address_object).collect::<Vec<_>>().join(","))
}
pub(crate) fn display_array(addresses: &[Address]) -> String {
    format!("[{}]", addresses.iter().map(|a| crate::json::string(&a.display())).collect::<Vec<_>>().join(","))
}
