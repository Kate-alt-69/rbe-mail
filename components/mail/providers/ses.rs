use rbe_sdk::RbeSdk;
use crate::http;
use crate::json;
use crate::types::{Address,DeliveryStatus,MailError,Message,ProviderKind,SendReceipt};
use super::sigv4;

fn arr(items:&[Address])->String{
    format!("[{}]",items.iter().map(|a|json::string(&a.email)).collect::<Vec<_>>().join(","))
}
pub fn send(
    sdk:RbeSdk<'_>,access:&str,secret:&str,token:Option<&str>,region:&str,
    from:&Address,to:&[Address],m:&Message
)->Result<SendReceipt,MailError>{
    let mut body_parts=Vec::new();
    if let Some(v)=&m.text{body_parts.push(format!("\"Text\":{{\"Data\":{}}}",json::string(v)));}
    if let Some(v)=&m.html{body_parts.push(format!("\"Html\":{{\"Data\":{}}}",json::string(v)));}
    let payload=format!(
        "{{\"FromEmailAddress\":{},\"Destination\":{{\"ToAddresses\":{},\"CcAddresses\":{},\"BccAddresses\":{}}},\"Content\":{{\"Simple\":{{\"Subject\":{{\"Data\":{}}},\"Body\":{{{}}}}}}}}}",
        json::string(&from.email),arr(to),arr(&m.cc),arr(&m.bcc),json::string(&m.subject),body_parts.join(",")
    );
    let host=format!("email.{region}.amazonaws.com");
    let uri="/v2/email/outbound-emails";
    let mut headers=sigv4::sign(access,secret,token,region,&host,"POST",uri,"application/json",payload.as_bytes())
        .map_err(MailError::network)?;
    headers.push(("Content-Type","application/json".into()));
    let response=http::post_json(sdk,ProviderKind::Ses,&format!("https://{host}{uri}"),&headers,payload)?;
    Ok(SendReceipt{provider:ProviderKind::Ses,message_id:json::find_string_field(&response.body,"MessageId"),status:DeliveryStatus::Accepted})
}
