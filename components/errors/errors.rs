#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct ErrorInfo{pub code:&'static str,pub summary:&'static str}
pub const ERRORS:&[ErrorInfo]=&[
    ErrorInfo{code:"MAIL1001",summary:"Invalid email address"},
    ErrorInfo{code:"MAIL1002",summary:"Invalid message"},
    ErrorInfo{code:"MAIL1003",summary:"Invalid mail domain"},
    ErrorInfo{code:"MAIL1004",summary:"Invalid transport configuration"},
    ErrorInfo{code:"MAIL1005",summary:"Invalid provider API path"},
    ErrorInfo{code:"MAIL1006",summary:"Unsupported provider HTTP method"},
    ErrorInfo{code:"MAIL1007",summary:"Provider credential missing"},
    ErrorInfo{code:"MAIL1016",summary:"Header injection rejected"},
    ErrorInfo{code:"MAIL2001",summary:"Required RBE capability missing"},
    ErrorInfo{code:"MAIL2002",summary:"Self-hosted server prerequisite missing"},
    ErrorInfo{code:"MAIL2003",summary:"TCP capability missing"},
    ErrorInfo{code:"MAIL2004",summary:"TLS capability missing"},
    ErrorInfo{code:"MAIL2005",summary:"DNS capability missing"},
    ErrorInfo{code:"MAIL4007",summary:"RBE mail network operation failed"},
    ErrorInfo{code:"MAIL4013",summary:"DNS lookup failed"},
    ErrorInfo{code:"MAIL4014",summary:"No MX records"},
    ErrorInfo{code:"MAIL8001",summary:"Malformed external or host response"},
    ErrorInfo{code:"MAIL8002",summary:"Malformed DNS response"},
    ErrorInfo{code:"MAIL9001",summary:"Mail package invariant violated"},
    ErrorInfo{code:"MAIL9002",summary:"Invalid system clock"},
];
pub fn explain(code:&str)->Option<&'static ErrorInfo>{ERRORS.iter().find(|x|x.code.eq_ignore_ascii_case(code))}
