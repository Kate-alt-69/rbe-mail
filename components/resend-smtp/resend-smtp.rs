#[derive(Debug,Clone,PartialEq,Eq)]
pub struct SmtpProfile{pub host:String,pub port:u16,pub username:String,pub password:String,pub security:&'static str}
pub fn profile(password:impl Into<String>)->Result<SmtpProfile,String>{
    let password=password.into();
    if password.is_empty(){return Err("MAIL1014: SMTP credential cannot be empty".into());}
    Ok(SmtpProfile{host:"smtp.resend.com".into(),port:465,username:"resend".into(),password,security:"tls-wrapper"})
}
pub const CREDENTIAL_NOTE:&str="API key";
