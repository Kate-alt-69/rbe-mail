#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct ProviderInfo{
    pub name:&'static str,
    pub api_component:&'static str,
    pub smtp_component:&'static str,
    pub legacy_component:Option<&'static str>,
}
pub const PROVIDERS:&[ProviderInfo]=&[
    ProviderInfo{name:"resend",api_component:"resend-api",smtp_component:"resend-smtp",legacy_component:None},
    ProviderInfo{name:"sendgrid",api_component:"sendgrid-api",smtp_component:"sendgrid-smtp",legacy_component:Some("sendgrid-legacy")},
    ProviderInfo{name:"brevo",api_component:"brevo-api",smtp_component:"brevo-smtp",legacy_component:None},
    ProviderInfo{name:"postmark",api_component:"postmark-api",smtp_component:"postmark-smtp",legacy_component:None},
    ProviderInfo{name:"mailgun",api_component:"mailgun-api",smtp_component:"mailgun-smtp",legacy_component:None},
    ProviderInfo{name:"sendpulse",api_component:"sendpulse-api",smtp_component:"sendpulse-smtp",legacy_component:None},
    ProviderInfo{name:"ses",api_component:"ses-api",smtp_component:"ses-smtp",legacy_component:Some("ses-legacy-api")},
];
pub fn find(name:&str)->Option<&'static ProviderInfo>{
    PROVIDERS.iter().find(|p|p.name.eq_ignore_ascii_case(name))
}
