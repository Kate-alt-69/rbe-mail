#[derive(Debug,Clone,PartialEq,Eq)]
pub struct SmtpProfile{pub host:String,pub port:u16,pub username:String,pub password:String,pub security:&'static str}
pub fn profile(username:impl Into<String>,password:impl Into<String>)->Result<SmtpProfile,String>{
    let username=username.into();let password=password.into();
    if username.is_empty()||password.is_empty(){return Err("MAIL1014: SMTP credentials cannot be empty".into());}
    Ok(SmtpProfile{host:"smtp-pulse.com".into(),port:587,username,password,security:"starttls"})
}
pub const CREDENTIAL_NOTE:&str="SMTP password";
