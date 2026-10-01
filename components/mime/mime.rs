fn header(value:&str)->Result<(),String>{
    if value.contains('\r')||value.contains('\n'){Err("MAIL1016: CR/LF header injection rejected".into())}else{Ok(())}
}
pub fn text_message(from:&str,to:&str,subject:&str,message_id:&str,body:&str)->Result<String,String>{
    for v in [from,to,subject,message_id]{header(v)?}
    Ok(format!("From: {from}\r\nTo: {to}\r\nSubject: {subject}\r\nMessage-ID: {message_id}\r\nMIME-Version: 1.0\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: 8bit\r\n\r\n{body}"))
}
pub fn multipart_alternative(from:&str,to:&str,subject:&str,message_id:&str,text:&str,html:&str)->Result<String,String>{
    for v in [from,to,subject,message_id]{header(v)?}
    let b=format!("rbe-mail-{:x}",message_id.bytes().fold(0u64,|a,x|a.wrapping_mul(131).wrapping_add(x as u64)));
    Ok(format!("From: {from}\r\nTo: {to}\r\nSubject: {subject}\r\nMessage-ID: {message_id}\r\nMIME-Version: 1.0\r\nContent-Type: multipart/alternative; boundary=\"{b}\"\r\n\r\n--{b}\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{text}\r\n--{b}\r\nContent-Type: text/html; charset=utf-8\r\n\r\n{html}\r\n--{b}--\r\n"))
}
