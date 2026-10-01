use rbe_sdk::{HostBridge,RbeSdk};

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum Security{StartTls,TlsWrapper}

#[derive(Debug,Clone,PartialEq,Eq)]
pub struct SmtpProfile{
    pub host:String,pub port:u16,pub username:String,pub password:String,pub security:Security
}

impl SmtpProfile{
    pub fn new(host:impl Into<String>,port:u16,username:impl Into<String>,password:impl Into<String>,security:Security)->Result<Self,String>{
        let host=host.into();let username=username.into();let password=password.into();
        if host.trim().is_empty()||port==0{return Err("MAIL1013: invalid SMTP endpoint".into());}
        if username.is_empty()||password.is_empty(){return Err("MAIL1014: SMTP credentials cannot be empty".into());}
        Ok(Self{host,port,username,password,security})
    }
}

#[derive(Debug,Clone,PartialEq,Eq)]
pub struct Capabilities{pub tcp:bool,pub tls:bool}

pub fn capabilities(host:&dyn HostBridge)->Capabilities{
    let sdk=RbeSdk::new(host);
    Capabilities{
        tcp:sdk.host().granted("net:tcp").unwrap_or(false),
        tls:sdk.host().granted("net:tls").unwrap_or(false),
    }
}
pub fn require_secure_transport(host:&dyn HostBridge)->Result<(),String>{
    let c=capabilities(host);
    if !c.tcp{return Err("MAIL2003: RBE net:tcp capability is not granted".into());}
    if !c.tls{return Err("MAIL2004: RBE TLS/STARTTLS capability is not granted".into());}
    Ok(())
}
