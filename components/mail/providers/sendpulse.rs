use rbe_sdk::RbeSdk;
use crate::http;
use crate::json;
use crate::types::{Address,DeliveryStatus,MailError,Message,ProviderKind,SendReceipt};

fn b64(input:&[u8])->String{
    const T:&[u8;64]=b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out=String::new();
    for c in input.chunks(3){
        let a=c[0];let b=*c.get(1).unwrap_or(&0);let d=*c.get(2).unwrap_or(&0);
        out.push(T[(a>>2)as usize]as char);
        out.push(T[(((a&3)<<4)|(b>>4))as usize]as char);
        if c.len()>1{out.push(T[(((b&15)<<2)|(d>>6))as usize]as char)}else{out.push('=')}
        if c.len()>2{out.push(T[(d&63)as usize]as char)}else{out.push('=')}
    }out
}
fn addr(a:&Address)->String{
    format!("{{\"email\":{},\"name\":{}}}",json::string(&a.email),json::string(a.name.as_deref().unwrap_or("")))
}
fn arr(items:&[Address])->String{
    format!("[{}]",items.iter().map(addr).collect::<Vec<_>>().join(","))
}
pub fn send(sdk:RbeSdk<'_>,token:&str,from:&Address,to:&[Address],m:&Message)->Result<SendReceipt,MailError>{
    let mut f=vec![
        format!("\"subject\":{}",json::string(&m.subject)),
        format!("\"from\":{}",addr(from)),
        format!("\"to\":{}",arr(to)),
    ];
    if let Some(v)=&m.text{f.push(format!("\"text\":{}",json::string(v)));}
    if let Some(v)=&m.html{f.push(format!("\"html\":{}",json::string(&b64(v.as_bytes()))));}
    if !m.cc.is_empty(){f.push(format!("\"cc\":{}",arr(&m.cc)));}
    if !m.bcc.is_empty(){f.push(format!("\"bcc\":{}",arr(&m.bcc)));}
    let response=http::post_json(
        sdk,ProviderKind::SendPulse,"https://api.sendpulse.com/smtp/emails",
        &[("Authorization",format!("Bearer {token}"))],
        format!("{{\"email\":{{{}}}}}",f.join(","))
    )?;
    Ok(SendReceipt{provider:ProviderKind::SendPulse,message_id:json::find_string_field(&response.body,"id"),status:DeliveryStatus::Accepted})
}
