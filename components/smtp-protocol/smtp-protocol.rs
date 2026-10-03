const MAX_COMMAND_BYTES: usize = 512;
const MAX_RECIPIENTS: usize = 100;
const MAX_DOMAIN_BYTES: usize = 253;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Ehlo(String),
    Helo(String),
    MailFrom(Option<String>),
    RcptTo(String),
    Data,
    Rset,
    Noop,
    StartTls,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    None,
    BeginData,
    StartTls,
    Close,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub code: u16,
    pub lines: Vec<String>,
    pub action: Action,
}

impl Reply {
    pub fn single(code: u16, text: impl Into<String>) -> Self {
        Self { code, lines: vec![text.into()], action: Action::None }
    }

    pub fn with_action(mut self, action: Action) -> Self {
        self.action = action;
        self
    }

    pub fn wire(&self) -> String {
        let mut out = String::new();
        for (index, line) in self.lines.iter().enumerate() {
            let separator = if index + 1 == self.lines.len() { ' ' } else { '-' };
            out.push_str(&format!("{}{}{}\r\n", self.code, separator, line));
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Policy {
    local_domains: Vec<String>,
    pub authenticated_relay: bool,
    pub starttls_available: bool,
    pub max_message_bytes: usize,
}

impl Policy {
    pub fn new(local_domains: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut domains = Vec::new();
        for domain in local_domains {
            let domain = normalize_domain(&domain)?;
            if !domains.iter().any(|value| value == &domain) {
                domains.push(domain);
            }
        }
        if domains.is_empty() {
            return Err("MAIL1003: SMTP server policy requires at least one local recipient domain".into());
        }
        Ok(Self {
            local_domains: domains,
            authenticated_relay: false,
            starttls_available: true,
            max_message_bytes: 25 * 1024 * 1024,
        })
    }

    pub fn local_domains(&self) -> &[String] {
        &self.local_domains
    }

    pub fn allows_recipient(&self, address: &str) -> bool {
        if self.authenticated_relay {
            return true;
        }
        let Some((_, domain)) = split_mailbox(address) else {
            return false;
        };
        self.local_domains.iter().any(|local| local == domain)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    greeted: bool,
    tls_active: bool,
    mail_started: bool,
    mail_from: Option<String>,
    recipients: Vec<String>,
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    pub const fn new() -> Self {
        Self {
            greeted: false,
            tls_active: false,
            mail_started: false,
            mail_from: None,
            recipients: Vec::new(),
        }
    }

    pub fn tls_active(&self) -> bool {
        self.tls_active
    }

    pub fn mail_from(&self) -> Option<&str> {
        self.mail_from.as_deref()
    }

    pub fn has_null_reverse_path(&self) -> bool {
        self.mail_started && self.mail_from.is_none()
    }

    pub fn recipients(&self) -> &[String] {
        &self.recipients
    }

    pub fn apply(&mut self, command: Command, policy: &Policy) -> Reply {
        match command {
            Command::Ehlo(identity) => {
                if !valid_helo_identity(&identity) {
                    return Reply::single(501, "Invalid EHLO identity");
                }
                self.reset_transaction();
                self.greeted = true;
                let mut lines = vec![
                    "RBE mail ready".to_string(),
                    format!("SIZE {}", policy.max_message_bytes),
                    "8BITMIME".to_string(),
                ];
                if policy.starttls_available && !self.tls_active {
                    lines.push("STARTTLS".into());
                }
                lines.push("HELP".into());
                Reply { code: 250, lines, action: Action::None }
            }
            Command::Helo(identity) => {
                if !valid_helo_identity(&identity) {
                    return Reply::single(501, "Invalid HELO identity");
                }
                self.reset_transaction();
                self.greeted = true;
                Reply::single(250, "RBE mail ready")
            }
            Command::MailFrom(sender) => {
                if !self.greeted {
                    return Reply::single(503, "Send HELO/EHLO first");
                }
                if let Some(ref address) = sender {
                    if !valid_mailbox(address) {
                        return Reply::single(501, "Invalid reverse-path");
                    }
                }
                self.mail_started = true;
                self.mail_from = sender;
                self.recipients.clear();
                Reply::single(250, "Sender accepted")
            }
            Command::RcptTo(recipient) => {
                if !self.mail_started {
                    return Reply::single(503, "Send MAIL FROM first");
                }
                if !valid_mailbox(&recipient) {
                    return Reply::single(501, "Invalid recipient mailbox");
                }
                if self.recipients.len() >= MAX_RECIPIENTS {
                    return Reply::single(452, "Too many recipients");
                }
                if !policy.allows_recipient(&recipient) {
                    return Reply::single(550, "Relaying denied");
                }
                if !self.recipients.iter().any(|value| value.eq_ignore_ascii_case(&recipient)) {
                    self.recipients.push(recipient);
                }
                Reply::single(250, "Recipient accepted")
            }
            Command::Data => {
                if !self.mail_started || self.recipients.is_empty() {
                    return Reply::single(503, "Need MAIL FROM and RCPT TO first");
                }
                Reply::single(354, "End data with <CRLF>.<CRLF>").with_action(Action::BeginData)
            }
            Command::Rset => {
                self.reset_transaction();
                Reply::single(250, "Transaction reset")
            }
            Command::Noop => Reply::single(250, "OK"),
            Command::StartTls => {
                if !self.greeted {
                    return Reply::single(503, "Send EHLO first");
                }
                if self.tls_active {
                    return Reply::single(503, "TLS already active");
                }
                if !policy.starttls_available {
                    return Reply::single(454, "TLS not available");
                }
                Reply::single(220, "Ready to start TLS").with_action(Action::StartTls)
            }
            Command::Quit => Reply::single(221, "Bye").with_action(Action::Close),
        }
    }

    /// Call only after the trusted RBE listener broker reports a successful TLS
    /// upgrade. RFC 3207 requires SMTP knowledge learned before STARTTLS to be
    /// discarded, so the client must EHLO/HELO again.
    pub fn mark_tls_active(&mut self) {
        self.tls_active = true;
        self.greeted = false;
        self.reset_transaction();
    }

    /// Call after the DATA body has been accepted or permanently rejected.
    pub fn finish_transaction(&mut self) {
        self.reset_transaction();
    }

    fn reset_transaction(&mut self) {
        self.mail_started = false;
        self.mail_from = None;
        self.recipients.clear();
    }
}

pub fn parse_command(line: &[u8]) -> Result<Command, String> {
    if line.is_empty() || line.len() > MAX_COMMAND_BYTES {
        return Err(format!("MAIL1002: SMTP command line must contain 1..={MAX_COMMAND_BYTES} bytes"));
    }
    if line.iter().any(|byte| *byte == 0 || (*byte < 0x20 && !matches!(*byte, b'\r' | b'\n' | b'\t'))) {
        return Err("MAIL1002: SMTP command contains forbidden control bytes".into());
    }
    let text = std::str::from_utf8(line)
        .map_err(|_| "MAIL1002: SMTP command must be valid UTF-8/ASCII command text".to_string())?;
    let text = text.strip_suffix("\r\n").or_else(|| text.strip_suffix('\n')).unwrap_or(text);
    if text.contains('\r') || text.contains('\n') {
        return Err("MAIL1002: SMTP command contains embedded line breaks".into());
    }
    let (verb, argument) = text
        .split_once(|ch: char| ch.is_ascii_whitespace())
        .map(|(verb, rest)| (verb, rest.trim()))
        .unwrap_or((text, ""));
    match verb.to_ascii_uppercase().as_str() {
        "EHLO" if !argument.is_empty() => Ok(Command::Ehlo(argument.to_string())),
        "HELO" if !argument.is_empty() => Ok(Command::Helo(argument.to_string())),
        "MAIL" => parse_path_command(argument, "FROM:", true).map(Command::MailFrom),
        "RCPT" => parse_path_command(argument, "TO:", false).and_then(|value| {
            value.map(Command::RcptTo).ok_or_else(|| "MAIL1001: RCPT TO cannot use the null path".into())
        }),
        "DATA" if argument.is_empty() => Ok(Command::Data),
        "RSET" if argument.is_empty() => Ok(Command::Rset),
        "NOOP" => Ok(Command::Noop),
        "STARTTLS" if argument.is_empty() => Ok(Command::StartTls),
        "QUIT" if argument.is_empty() => Ok(Command::Quit),
        _ => Err("MAIL1002: unsupported or malformed SMTP command".into()),
    }
}

fn parse_path_command(argument: &str, prefix: &str, allow_null: bool) -> Result<Option<String>, String> {
    if argument.len() < prefix.len() || !argument[..prefix.len()].eq_ignore_ascii_case(prefix) {
        return Err("MAIL1002: malformed SMTP envelope command".into());
    }
    let remainder = argument[prefix.len()..].trim_start();
    if !remainder.starts_with('<') {
        return Err("MAIL1001: SMTP envelope path must be enclosed in angle brackets".into());
    }
    let close = remainder
        .find('>')
        .ok_or_else(|| "MAIL1001: SMTP envelope path is missing closing angle bracket".to_string())?;
    let address = &remainder[1..close];
    let parameters = remainder[close + 1..].trim();
    if parameters.contains('\r') || parameters.contains('\n') {
        return Err("MAIL1002: SMTP envelope parameters contain line breaks".into());
    }
    if address.is_empty() {
        if allow_null {
            return Ok(None);
        }
        return Err("MAIL1001: recipient path cannot be empty".into());
    }
    if !valid_mailbox(address) {
        return Err("MAIL1001: invalid SMTP mailbox".into());
    }
    Ok(Some(address.to_string()))
}

fn valid_helo_identity(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_DOMAIN_BYTES
        && !value.chars().any(|ch| ch.is_control() || ch.is_whitespace())
}

fn valid_mailbox(value: &str) -> bool {
    let Some((local, domain)) = split_mailbox(value) else {
        return false;
    };
    !local.is_empty()
        && local.len() <= 64
        && local.bytes().all(|byte| byte > 0x20 && byte < 0x7f && !matches!(byte, b'<' | b'>' | b'(' | b')' | b',' | b';' | b':' | b'\\' | b'"' | b'[' | b']'))
        && valid_domain(domain)
}

fn split_mailbox(value: &str) -> Option<(&str, &str)> {
    let (local, domain) = value.rsplit_once('@')?;
    Some((local, domain))
}

fn normalize_domain(value: &str) -> Result<String, String> {
    let value = value.trim_end_matches('.').to_ascii_lowercase();
    if valid_domain(&value) {
        Ok(value)
    } else {
        Err("MAIL1003: invalid local SMTP domain".into())
    }
}

fn valid_domain(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_DOMAIN_BYTES
        && value.contains('.')
        && value.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> Policy {
        Policy::new(["example.com".to_string()]).unwrap()
    }

    #[test]
    fn parser_supports_null_reverse_path_but_not_null_recipient() {
        assert_eq!(parse_command(b"MAIL FROM:<>\r\n").unwrap(), Command::MailFrom(None));
        assert!(parse_command(b"RCPT TO:<>\r\n").is_err());
    }

    #[test]
    fn unauthenticated_session_cannot_relay_to_external_domain() {
        let mut session = Session::new();
        assert_eq!(session.apply(Command::Ehlo("sender.example".into()), &policy()).code, 250);
        assert_eq!(session.apply(Command::MailFrom(Some("sender@outside.test".into())), &policy()).code, 250);
        assert_eq!(session.apply(Command::RcptTo("user@outside.test".into()), &policy()).code, 550);
        assert_eq!(session.apply(Command::RcptTo("user@example.com".into()), &policy()).code, 250);
    }

    #[test]
    fn starttls_resets_smtp_state_after_successful_upgrade() {
        let mut session = Session::new();
        let policy = policy();
        session.apply(Command::Ehlo("sender.example".into()), &policy);
        session.apply(Command::MailFrom(Some("sender@outside.test".into())), &policy);
        let start = session.apply(Command::StartTls, &policy);
        assert_eq!(start.code, 220);
        assert_eq!(start.action, Action::StartTls);
        session.mark_tls_active();
        assert!(session.tls_active());
        assert!(session.recipients().is_empty());
        assert_eq!(session.apply(Command::MailFrom(None), &policy).code, 503);
    }

    #[test]
    fn data_requires_sender_and_recipient() {
        let mut session = Session::new();
        let policy = policy();
        session.apply(Command::Ehlo("sender.example".into()), &policy);
        assert_eq!(session.apply(Command::Data, &policy).code, 503);
        session.apply(Command::MailFrom(None), &policy);
        assert_eq!(session.apply(Command::Data, &policy).code, 503);
        session.apply(Command::RcptTo("postmaster@example.com".into()), &policy);
        assert_eq!(session.apply(Command::Data, &policy).code, 354);
    }

    #[test]
    fn wire_reply_uses_multiline_smtp_framing() {
        let mut session = Session::new();
        let reply = session.apply(Command::Ehlo("sender.example".into()), &policy());
        let wire = reply.wire();
        assert!(wire.starts_with("250-RBE mail ready\r\n"));
        assert!(wire.ends_with("250 HELP\r\n"));
    }
}
