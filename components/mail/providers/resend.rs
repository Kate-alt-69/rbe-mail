use rbe_sdk::RbeSdk;
use crate::{http, json};
use crate::types::{Address, DeliveryStatus, MailError, Message, ProviderKind, SendReceipt};
pub fn send(sdk: RbeSdk<'_>, api_key: &str, from: &Address, to: &[Address], message: &Message) -> Result<SendReceipt, MailError> {
    let mut fields = vec![format!("\"from\":{}", json::string(&from.display())), format!("\"to\":{}", super::display_array(to)), format!("\"subject\":{}", json::string(&message.subject))];
    if let Some(text) = &message.text { fields.push(format!("\"text\":{}", json::string(text))); }
    if let Some(html) = &message.html { fields.push(format!("\"html\":{}", json::string(html))); }
    if !message.cc.is_empty() { fields.push(format!("\"cc\":{}", super::display_array(&message.cc))); }
    if !message.bcc.is_empty() { fields.push(format!("\"bcc\":{}", super::display_array(&message.bcc))); }
    if let Some(reply_to) = &message.reply_to { fields.push(format!("\"reply_to\":{}", json::string(&reply_to.display()))); }
    let response = http::post_json(sdk, ProviderKind::Resend, "https://api.resend.com/emails", &[("Authorization", format!("Bearer {api_key}"))], format!("{{{}}}", fields.join(",")))?;
    Ok(SendReceipt { provider: ProviderKind::Resend, message_id: json::find_string_field(&response.body, "id"), status: DeliveryStatus::Accepted })
}
