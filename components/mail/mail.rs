mod http; mod json; mod providers; mod self_hosted; mod types;
use rbe_sdk::{AbiRange, HostBridge, LibraryDescriptor, RbeSdk};
pub use self_hosted::{MxRecord, SelfHostedCapabilities};
pub use types::{error_codes, Address, DeliveryStatus, MailError, MailErrorKind, MailgunRegion, Message, ProviderKind, SendReceipt, Transport};

pub const DESCRIPTOR: LibraryDescriptor<'static> = LibraryDescriptor { name: "mail", version: "0.1.0", abi: AbiRange::exact(1) };
pub struct Mail<'a> { sdk: RbeSdk<'a>, transport: Transport }
impl<'a> Mail<'a> {
    pub fn new(host: &'a dyn HostBridge, transport: Transport) -> Result<Self, MailError> {
        DESCRIPTOR.validate().map_err(|error| MailError::invalid_config(format!("invalid RBE SDK descriptor: {error}")))?;
        transport.validate()?; let sdk=RbeSdk::new(host);
        let log=sdk.log("mail").map_err(|error| MailError::capability("log"))?; let _=log.debug("mail package initialized");
        Ok(Self{sdk,transport})
    }
    pub fn transport(&self)->&Transport{&self.transport}
    pub fn send(&self,to:impl Into<Address>,message:Message)->Result<SendReceipt,MailError>{self.send_many(vec![to.into()],message)}
    pub fn send_many(&self,to:Vec<Address>,message:Message)->Result<SendReceipt,MailError>{
        if to.is_empty(){return Err(MailError::invalid_message("email needs at least one recipient"));}
        for address in &to{address.validate()?;} message.validate()?;
        if matches!(self.transport,Transport::SelfHosted{..}){self_hosted::require_secure_outbound(self.sdk)?;return Err(MailError::unsupported(error_codes::SERVER_PREREQUISITE_MISSING,"direct SMTP state machine is not enabled until the remaining RBE server primitives are available"));}
        if let Ok(log)=self.sdk.log("mail").and_then(|log|log.child("send")){let label=self.transport.provider_kind().map(ProviderKind::as_str).unwrap_or("self");let _=log.debug(format!("dispatching message through {label}"));}
        providers::send(self.sdk,&self.transport,&to,&message)
    }
    pub fn self_hosted_capabilities(&self)->SelfHostedCapabilities{self_hosted::capabilities(self.sdk)}
    pub fn resolve_mx(&self,domain:&str)->Result<Vec<MxRecord>,MailError>{self_hosted::resolve_mx(self.sdk,domain)}
    pub fn require_server_mode(&self)->Result<(),MailError>{self_hosted::require_server_mode(self.sdk)}
}
impl From<&str> for Address{fn from(value:&str)->Self{Address::new(value)}}
impl From<String> for Address{fn from(value:String)->Self{Address::new(value)}}
pub fn create_library<'a>(host:&'a dyn HostBridge,transport:Transport)->Result<Mail<'a>,MailError>{Mail::new(host,transport)}
