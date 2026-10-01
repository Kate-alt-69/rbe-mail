use rbe_sdk::RbeSdk;
use crate::{http, json};
use crate::types::{Address, DeliveryStatus, MailError, MailgunRegion, Message, ProviderKind, SendReceipt};
pub fn send(sdk: RbeSdk<'_>, api_key: &str, domain: &str, from: &Address, region: MailgunRegion, to: &[Address], message: &Message) -> Result<SendReceipt, MailError> {
    let boundary = "rbe-mail-boundary-v1";
    let endpoint = match region { MailgunRegion::Us => format!("https://api.mailgun.net/v3/{domain}/messages"), MailgunRegion::Eu => format!("https://api.eu.mailgun.net/v3/{domain}/messages") };
    let mut body = String::new(); push_field(&mut body,boundary,"from",&from.display());
    for a in to { push_field(&mut body,boundary,"to",&a.display()); } for a in &message.cc { push_field(&mut body,boundary,"cc",&a.display()); } for a in &message.bcc { push_field(&mut body,boundary,"bcc",&a.display()); }
    push_field(&mut body,boundary,"subject",&message.subject); if let Some(text)=&message.text { push_field(&mut body,boundary,"text",text); } if let Some(html)=&message.html { push_field(&mut body,boundary,"html",html); }
    if let Some(reply)=&message.reply_to { push_field(&mut body,boundary,"h:Reply-To",&reply.display()); } body.push_str(&format!("--{boundary}--\r\n"));
    let response = http::post_raw(sdk, ProviderKind::Mailgun, &endpoint, &format!("multipart/form-data; boundary={boundary}"), &[("Authorization", format!("Basic {}", base64(format!("api:{api_key}").as_bytes())))], body)?;
    Ok(SendReceipt { provider: ProviderKind::Mailgun, message_id: json::find_string_field(&response.body,"id"), status: DeliveryStatus::Accepted })
}
fn push_field(body:&mut String,boundary:&str,name:&str,value:&str){ body.push_str(&format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{}\"\r\n\r\n{}\r\n",name.replace('"',""),value)); }
fn base64(input:&[u8])->String{ const T:&[u8;64]=b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"; let mut out=String::with_capacity(input.len().div_ceil(3)*4); for c in input.chunks(3){let a=c[0];let b=*c.get(1).unwrap_or(&0);let d=*c.get(2).unwrap_or(&0);out.push(T[(a>>2)as usize]as char);out.push(T[(((a&3)<<4)|(b>>4))as usize]as char);if c.len()>1{out.push(T[(((b&15)<<2)|(d>>6))as usize]as char)}else{out.push('=')}if c.len()>2{out.push(T[(d&63)as usize]as char)}else{out.push('=')}}out}
#[cfg(test)] mod tests{use super::*;#[test]fn basic_auth_base64(){assert_eq!(base64(b"api:key"),"YXBpOmtleQ==");}}
