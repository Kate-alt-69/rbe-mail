use rbe_sdk::RbeSdk;
use crate::{http, json};
use crate::types::{Address, DeliveryStatus, MailError, Message, ProviderKind, SendReceipt};
pub fn send(sdk: RbeSdk<'_>, api_key: &str, from: &Address, to: &[Address], message: &Message) -> Result<SendReceipt, MailError> {
    let mut p = format!("{{\"to\":{}", super::address_array(to));
    if !message.cc.is_empty() { p.push_str(&format!(",\"cc\":{}", super::address_array(&message.cc))); }
    if !message.bcc.is_empty() { p.push_str(&format!(",\"bcc\":{}", super::address_array(&message.bcc))); } p.push('}');
    let mut content = Vec::new();
    if let Some(text) = &message.text { content.push(format!("{{\"type\":\"text/plain\",\"value\":{}}}", json::string(text))); }
    if let Some(html) = &message.html { content.push(format!("{{\"type\":\"text/html\",\"value\":{}}}", json::string(html))); }
    let mut fields = vec![format!("\"personalizations\":[{p}]"), format!("\"from\":{}", super::address_object(from)), format!("\"subject\":{}", json::string(&message.subject)), format!("\"content\":[{}]", content.join(","))];
    if let Some(reply_to) = &message.reply_to { fields.push(format!("\"reply_to\":{}", super::address_object(reply_to))); }
    http::post_json(sdk, ProviderKind::SendGrid, "https://api.sendgrid.com/v3/mail/send", &[("Authorization", format!("Bearer {api_key}"))], format!("{{{}}}", fields.join(",")))?;
    Ok(SendReceipt { provider: ProviderKind::SendGrid, message_id: None, status: DeliveryStatus::Accepted })
}
