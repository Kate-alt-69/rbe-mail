use rbe_sdk::RbeSdk;
use crate::{http, json};
use crate::types::{Address, DeliveryStatus, MailError, Message, ProviderKind, SendReceipt};
pub fn send(sdk: RbeSdk<'_>, server_token: &str, from: &Address, stream: Option<&str>, to: &[Address], message: &Message) -> Result<SendReceipt, MailError> {
    let joined = |xs: &[Address]| xs.iter().map(Address::display).collect::<Vec<_>>().join(",");
    let mut fields = vec![format!("\"From\":{}", json::string(&from.display())), format!("\"To\":{}", json::string(&joined(to))), format!("\"Subject\":{}", json::string(&message.subject))];
    if let Some(text) = &message.text { fields.push(format!("\"TextBody\":{}", json::string(text))); }
    if let Some(html) = &message.html { fields.push(format!("\"HtmlBody\":{}", json::string(html))); }
    if !message.cc.is_empty() { fields.push(format!("\"Cc\":{}", json::string(&joined(&message.cc)))); }
    if !message.bcc.is_empty() { fields.push(format!("\"Bcc\":{}", json::string(&joined(&message.bcc)))); }
    if let Some(reply_to) = &message.reply_to { fields.push(format!("\"ReplyTo\":{}", json::string(&reply_to.display()))); }
    if let Some(stream) = stream.filter(|s| !s.trim().is_empty()) { fields.push(format!("\"MessageStream\":{}", json::string(stream))); }
    let response = http::post_json(sdk, ProviderKind::Postmark, "https://api.postmarkapp.com/email", &[("X-Postmark-Server-Token", server_token.to_string())], format!("{{{}}}", fields.join(",")))?;
    Ok(SendReceipt { provider: ProviderKind::Postmark, message_id: json::find_string_field(&response.body, "MessageID"), status: DeliveryStatus::Accepted })
}
