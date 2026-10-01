#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyTemplate { pub template_id:String }

impl LegacyTemplate {
    pub fn new(template_id:impl Into<String>)->Result<Self,String>{
        let template_id=template_id.into();
        if template_id.trim().is_empty(){return Err("MAIL1012: SendGrid legacy template id cannot be empty".into());}
        Ok(Self{template_id})
    }
    pub fn x_smtpapi_header(&self)->String{
        let id=self.template_id.replace('\\',"\\\\").replace('"',"\\\"");
        format!("{{\"filters\":{{\"templates\":{{\"settings\":{{\"enable\":1,\"template_id\":\"{id}\"}}}}}}}}")
    }
    pub fn v3_template_id(&self)->&str{&self.template_id}
    pub const fn generation()->&'static str{"legacy"}
}
