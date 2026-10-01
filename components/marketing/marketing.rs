#[derive(Debug,Clone,PartialEq,Eq)]
pub struct UnsubscribeHeaders{
    pub list_unsubscribe:String,
    pub list_unsubscribe_post:Option<&'static str>,
}
pub fn unsubscribe(url:&str,one_click:bool)->Result<UnsubscribeHeaders,String>{
    if !(url.starts_with("https://")||url.starts_with("mailto:"))||url.contains('\r')||url.contains('\n'){
        return Err("MAIL1002: invalid List-Unsubscribe target".into());
    }
    Ok(UnsubscribeHeaders{
        list_unsubscribe:format!("<{url}>"),
        list_unsubscribe_post:if one_click{Some("List-Unsubscribe=One-Click")}else{None},
    })
}
