fn valid(value:&str)->bool{!value.trim().is_empty()&&!value.contains('\r')&&!value.contains('\n')}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct ThreadHeaders{pub in_reply_to:String,pub references:String}
pub fn reply(parent_message_id:&str,existing_references:Option<&str>)->Result<ThreadHeaders,String>{
    if !valid(parent_message_id){return Err("MAIL1002: invalid parent Message-ID".into());}
    let references=match existing_references{
        Some(v) if valid(v)=>format!("{v} {parent_message_id}"),
        Some(_)=>return Err("MAIL1002: invalid References header".into()),
        None=>parent_message_id.to_string(),
    };
    Ok(ThreadHeaders{in_reply_to:parent_message_id.into(),references})
}
