pub fn render(template:&str,variables:&[(&str,&str)])->Result<String,String>{
    let mut out=template.to_string();
    for (key,value) in variables{
        if key.is_empty()||!key.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'_'||b==b'-'){
            return Err("MAIL1017: invalid template variable name".into());
        }
        out=out.replace(&format!("{{{{{key}}}}}"),value);
    }Ok(out)
}
pub fn unresolved(value:&str)->bool{value.contains("{{")&&value.contains("}}")}
