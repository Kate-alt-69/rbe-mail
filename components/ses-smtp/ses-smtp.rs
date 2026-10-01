#[derive(Debug,Clone,PartialEq,Eq)]
pub struct SmtpProfile{pub host:String,pub port:u16,pub username:String,pub password:String,pub security:&'static str}
pub fn profile(region:&str,username:impl Into<String>,password:impl Into<String>)->Result<SmtpProfile,String>{
    if region.trim().is_empty()||!region.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-'){
        return Err("MAIL1011: invalid AWS region".into());
    }
    let username=username.into();let password=password.into();
    if username.is_empty()||password.is_empty(){return Err("MAIL1014: SMTP credentials cannot be empty".into());}
    Ok(SmtpProfile{
        host:format!("email-smtp.{region}.amazonaws.com"),
        port:587,username,password,security:"starttls"
    })
}
pub const CREDENTIAL_NOTE:&str="SES SMTP credentials are not the same as AWS API access keys.";
