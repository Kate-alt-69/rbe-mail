use rbe_sdk::{HostBridge, HostCall, RbeSdk};
use std::net::Ipv4Addr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnsRecord {
    pub kind: &'static str,
    pub name: String,
    pub value: String,
    pub priority: Option<u16>,
}

fn validate_domain(value: &str) -> bool {
    let value = value.trim_end_matches('.');
    !value.is_empty()
        && value.len() <= 253
        && value.contains('.')
        && value.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label.as_bytes().first().is_some_and(u8::is_ascii_alphanumeric)
                && label.as_bytes().last().is_some_and(u8::is_ascii_alphanumeric)
                && label.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
}

fn forbidden_public_ipv4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_multicast()
        || ip == Ipv4Addr::BROADCAST
        || octets[0] == 0
        || (octets[0] == 100 && (64..=127).contains(&octets[1]))
        || (octets[0] == 192 && octets[1] == 0 && (octets[2] == 0 || octets[2] == 2))
        || (octets[0] == 198 && (octets[1] == 18 || octets[1] == 19))
        || (octets[0] == 198 && octets[1] == 51 && octets[2] == 100)
        || (octets[0] == 203 && octets[1] == 0 && octets[2] == 113)
        || octets[0] >= 240
}

fn validate_token(token: &str) -> bool {
    token.len() == 64 && token.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Build the self-hosted DNS plan using a caller-supplied 256-bit verification token.
///
/// Generate `verification_token` with a cryptographically secure RNG and pass it as
/// 64 hexadecimal characters. Do not derive it from timestamps, domains or counters.
pub fn setup_records_with_token(
    domain: &str,
    mail_host: &str,
    public_ip: &str,
    verification_token: &str,
) -> Result<Vec<DnsRecord>, String> {
    let domain = domain.trim_end_matches('.').to_ascii_lowercase();
    let mail_host = mail_host.trim_end_matches('.').to_ascii_lowercase();
    if !validate_domain(&domain) || !validate_domain(&mail_host) {
        return Err("MAIL1018: self-hosted domain/mail host must be fully-qualified ASCII DNS names".into());
    }
    let public_ip: Ipv4Addr = public_ip
        .parse()
        .map_err(|_| "MAIL1020: self-hosted public_ip must be a valid IPv4 address because this setup emits an A record and ip4 SPF term".to_string())?;
    if forbidden_public_ipv4(public_ip) {
        return Err("MAIL1020: self-hosted public_ip must be publicly routable; private/loopback/link-local/multicast addresses cannot receive Internet SMTP".into());
    }
    if !validate_token(verification_token) {
        return Err("MAIL1019: verification token must contain exactly 64 hexadecimal characters (256 bits); generate it with a cryptographically secure RNG".into());
    }

    Ok(vec![
        DnsRecord { kind: "TXT", name: format!("_rbe-mail.{domain}"), value: format!("rbe-mail-verification={}", verification_token.to_ascii_lowercase()), priority: None },
        DnsRecord { kind: "MX", name: domain.clone(), value: mail_host.clone(), priority: Some(10) },
        DnsRecord { kind: "A", name: mail_host, value: public_ip.to_string(), priority: None },
        DnsRecord { kind: "TXT", name: domain.clone(), value: format!("v=spf1 ip4:{public_ip} -all"), priority: None },
        DnsRecord { kind: "TXT", name: format!("_dmarc.{domain}"), value: "v=DMARC1; p=none".into(), priority: None },
    ])
}

/// Generate a cryptographically secure ownership token through the trusted RBE
/// crypto broker and immediately build the self-hosted DNS plan.
pub fn setup_records_secure(
    host: &dyn HostBridge,
    domain: &str,
    mail_host: &str,
    public_ip: &str,
) -> Result<Vec<DnsRecord>, String> {
    let sdk = RbeSdk::new(host);
    let Some(session) = sdk.host().session() else {
        return Err("MAIL2006: RBE Library Host session is unavailable; secure self-hosted setup requires an accepted host session".into());
    };
    if !session.granted("crypto") {
        return Err("MAIL2001: RBE crypto capability is not granted; approve crypto before generating a self-hosted ownership token".into());
    }
    let reply = host
        .call(HostCall::new(
            "crypto",
            "crypto",
            "random",
            br#"{"bytes":32}"#,
        ))
        .map_err(|error| format!("MAIL4007: RBE secure ownership-token generation failed: {error}"))?;
    let text = std::str::from_utf8(&reply.payload)
        .map_err(|_| "MAIL8001: RBE crypto random response was not UTF-8 JSON".to_string())?;
    let token = string_field(text, "data_hex")?;
    if !validate_token(&token) {
        return Err("MAIL8001: RBE crypto random response did not contain a 256-bit hexadecimal token".into());
    }
    setup_records_with_token(domain, mail_host, public_ip, &token)
}

/// Legacy convenience entry retained for source compatibility with the initial
/// package draft. It remains fail-closed because it has no HostBridge from which
/// to request cryptographically secure randomness. New code should call
/// `setup_records_secure(host, ...)` or `setup_records_with_token(...)`.
pub fn setup_records(domain: &str, mail_host: &str, public_ip: &str) -> Result<Vec<DnsRecord>, String> {
    let _ = (domain, mail_host, public_ip);
    Err("MAIL2007: secure verification-token generation requires an RBE HostBridge; call setup_records_secure(host, ...) or setup_records_with_token(...)".into())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capabilities {
    pub session: bool,
    pub dns: bool,
    pub tcp: bool,
    pub tls: bool,
    pub listen: bool,
    pub storage: bool,
    pub crypto: bool,
    pub package_service: bool,
}

pub fn capabilities(host: &dyn HostBridge) -> Capabilities {
    let sdk = RbeSdk::new(host);
    Capabilities {
        session: sdk.host().session().is_some(),
        dns: sdk.host().granted("net:dns") == Some(true),
        tcp: sdk.host().granted("net:tcp") == Some(true),
        tls: sdk.host().granted("net:tls") == Some(true),
        listen: sdk.host().granted("net:tcp-listen") == Some(true),
        storage: sdk.host().granted("storage") == Some(true),
        crypto: sdk.host().granted("crypto") == Some(true),
        package_service: sdk.host().granted("service:package") == Some(true),
    }
}

fn string_field(input: &str, field: &str) -> Result<String, String> {
    let needle = format!("\"{field}\"");
    let start = input
        .find(&needle)
        .ok_or_else(|| format!("MAIL8001: RBE response is missing field {field:?}"))?
        + needle.len();
    let tail = &input[start..];
    let colon = tail
        .find(':')
        .ok_or_else(|| format!("MAIL8001: RBE response field {field:?} has no value"))?;
    let mut chars = tail[colon + 1..].trim_start().chars();
    if chars.next() != Some('"') {
        return Err(format!("MAIL8001: RBE response field {field:?} is not a string"));
    }
    let mut out = String::new();
    let mut escaped = false;
    for ch in chars {
        if escaped {
            match ch {
                '"' => out.push('"'),
                '\\' => out.push('\\'),
                '/' => out.push('/'),
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                other => out.push(other),
            }
            escaped = false;
        } else {
            match ch {
                '\\' => escaped = true,
                '"' => return Ok(out),
                other => out.push(other),
            }
        }
    }
    Err(format!("MAIL8001: RBE response field {field:?} contains an unterminated string"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_predictable_or_short_verification_tokens() {
        assert!(setup_records_with_token("example.com", "mail.example.com", "1.1.1.1", "1234").is_err());
        assert!(setup_records("example.com", "mail.example.com", "1.1.1.1").is_err());
    }

    #[test]
    fn accepts_256_bit_hex_token() {
        let records = setup_records_with_token(
            "example.com",
            "mail.example.com",
            "1.1.1.1",
            &"ab".repeat(32),
        ).unwrap();
        assert!(records.iter().any(|record| record.name == "_rbe-mail.example.com"));
    }

    #[test]
    fn parses_secure_random_host_response() {
        let text = format!(r#"{{"data_hex":"{}"}}"#, "ab".repeat(32));
        assert_eq!(string_field(&text, "data_hex").unwrap(), "ab".repeat(32));
    }
}
