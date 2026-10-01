use rbe_sdk::{HostBridge,RbeSdk};
use std::time::{SystemTime,UNIX_EPOCH};

#[derive(Debug,Clone,PartialEq,Eq)]
pub struct DnsRecord{pub kind:&'static str,pub name:String,pub value:String,pub priority:Option<u16>}
fn token()->Result<String,String>{
    let n=SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_|"MAIL9002: invalid system clock")?.as_nanos();
    Ok(format!("{n:032x}"))
}
pub fn setup_records(domain:&str,mail_host:&str,public_ip:&str)->Result<Vec<DnsRecord>,String>{
    if domain.is_empty()||mail_host.is_empty()||public_ip.is_empty(){return Err("MAIL1018: self-hosted setup is incomplete".into());}
    let t=token()?;
    Ok(vec![
        DnsRecord{kind:"TXT",name:format!("_rbe-mail.{domain}"),value:format!("rbe-mail-verification={t}"),priority:None},
        DnsRecord{kind:"MX",name:domain.into(),value:mail_host.into(),priority:Some(10)},
        DnsRecord{kind:"A",name:mail_host.into(),value:public_ip.into(),priority:None},
        DnsRecord{kind:"TXT",name:domain.into(),value:format!("v=spf1 ip4:{public_ip} -all"),priority:None},
        DnsRecord{kind:"TXT",name:format!("_dmarc.{domain}"),value:"v=DMARC1; p=none".into(),priority:None},
    ])
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct Capabilities{pub dns:bool,pub tcp:bool,pub tls:bool,pub listen:bool,pub storage:bool,pub crypto:bool,pub package_service:bool}
pub fn capabilities(host:&dyn HostBridge)->Capabilities{
    let sdk=RbeSdk::new(host);let g=|n:&str|sdk.host().granted(n).unwrap_or(false);
    Capabilities{dns:g("net:dns"),tcp:g("net:tcp"),tls:g("net:tls"),listen:g("net:tcp-listen"),storage:g("storage"),crypto:g("crypto"),package_service:g("service:package")}
}
