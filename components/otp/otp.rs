#[derive(Debug,Clone,PartialEq,Eq)]
pub struct OtpMessage {
    pub subject:String,
    pub text:String,
    pub html:String,
    pub code:String,
    pub expires_in_minutes:u16,
}
pub fn build(product:&str,code:&str,expires_in_minutes:u16)->Result<OtpMessage,String>{
    if product.trim().is_empty()||code.trim().is_empty(){return Err("MAIL1002: OTP product/code cannot be empty".into());}
    if expires_in_minutes==0||expires_in_minutes>1440{return Err("MAIL1002: OTP expiry must be 1..1440 minutes".into());}
    if code.chars().any(char::is_control){return Err("MAIL1002: OTP code contains control characters".into());}
    let subject=format!("{product} verification code");
    let text=format!("Your {product} verification code is {code}. It expires in {expires_in_minutes} minutes.");
    let html=format!("<p>Your <strong>{product}</strong> verification code is:</p><p><strong>{code}</strong></p><p>It expires in {expires_in_minutes} minutes.</p>");
    Ok(OtpMessage{subject,text,html,code:code.into(),expires_in_minutes})
}
