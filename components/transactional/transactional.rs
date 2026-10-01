#[derive(Debug,Clone,PartialEq,Eq)]
pub struct Transactional {
    pub subject:String,
    pub text:Option<String>,
    pub html:Option<String>,
    pub idempotency_key:Option<String>,
}
impl Transactional{
    pub fn new(subject:impl Into<String>)->Result<Self,String>{
        let subject=subject.into();
        if subject.trim().is_empty()||subject.contains('\r')||subject.contains('\n'){return Err("MAIL1002: invalid transactional subject".into());}
        Ok(Self{subject,text:None,html:None,idempotency_key:None})
    }
    pub fn text(mut self,value:impl Into<String>)->Self{self.text=Some(value.into());self}
    pub fn html(mut self,value:impl Into<String>)->Self{self.html=Some(value.into());self}
    pub fn idempotency_key(mut self,value:impl Into<String>)->Result<Self,String>{
        let value=value.into();
        if value.trim().is_empty()||value.len()>256{return Err("MAIL1002: invalid idempotency key".into());}
        self.idempotency_key=Some(value);Ok(self)
    }
    pub fn validate(&self)->Result<(),String>{
        if self.text.as_deref().is_none_or(str::is_empty)&&self.html.as_deref().is_none_or(str::is_empty){
            return Err("MAIL1002: transactional email needs text or html".into());
        }Ok(())
    }
}
