use rbe_sdk::RbeSdk;
use crate::{http, json};
use crate::types::{Address, DeliveryStatus, MailError, Message, ProviderKind, SendReceipt};
pub fn send(sdk: RbeSdk<'_>, api_key: &str, from: &Address, to: &[Address], message: &Message) -> Result<SendReceipt, MailError> {
    let mut fields = vec![format!("\"sender\":{}", super::address_object(from)), format!("\"to\":{}", super::address_array(to)), format!("\"subject\":{}", json::string(&message.subject))];
    if let Some(text) = &message.text { fields.push(format!("\"textContent\":{}", json::string(text))); }
    if let Some(html) = &message.html { fields.push(format!("\"htmlContent\":{}", json::string(html))); }
    if !message.cc.is_empty() { fields.push(format!("\"cc\":{}", super::address_array(&message.cc))); }
    if !message.bcc.is_empty() { fields.push(format!("\"bcc\":{}", super::address_array(&message.bcc))); }
    if let Some(reply_to) = &message.reply_to { fields.push(format!("\"replyTo\":{}", super::address_object(reply_to))); }
    let response = http::post_json(sdk, ProviderKind::Brevo, "https://api.brevo.com/v3/smtp/email", &[("api-key", api_key.to_string())], format!("{{{}}}", fields.join(",")))?;
    Ok(SendReceipt { provider: ProviderKind::Brevo, message_id: json::find_string_field(&response.body, "messageId"), status: DeliveryStatus::Accepted })
}
