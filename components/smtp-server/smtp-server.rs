use rbe_sdk::{HostBridge, HostCall, HostReply, RbeSdk};

const LISTENER_CAPABILITY: &str = "net:tcp-listen";
const CRYPTO_CAPABILITY: &str = "crypto";
const MAX_READ_BYTES: usize = 16 * 1024;
const MAX_WRITE_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listener {
    pub handle: String,
    pub local: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connection {
    pub handle: String,
    pub peer: String,
    pub local: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadResult {
    pub data: Vec<u8>,
    pub eof: bool,
    pub tls: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlsResult {
    pub handle: String,
    pub tls: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ed25519Signature {
    pub public_key_hex: String,
    pub signature_hex: String,
}

pub struct SmtpServer<'a> {
    bridge: &'a dyn HostBridge,
}

impl<'a> SmtpServer<'a> {
    pub fn new(bridge: &'a dyn HostBridge) -> Result<Self, String> {
        let sdk = RbeSdk::new(bridge);
        let Some(session) = sdk.host().session() else {
            return Err("MAIL2006: RBE Library Host session is unavailable; smtp-server requires an accepted package host session".into());
        };
        if !session.granted(LISTENER_CAPABILITY) {
            return Err("MAIL4011: RBE net:tcp-listen is not granted; approve the mail package listener privilege before starting inbound SMTP".into());
        }
        Ok(Self { bridge })
    }

    pub fn bind(&self, address: &str, port: u16) -> Result<Listener, String> {
        if address.is_empty() || port == 0 {
            return Err("MAIL1013: SMTP listener address must be non-empty and port must be in 1..=65535".into());
        }
        let payload = format!(
            "{{\"address\":{},\"port\":{port}}}",
            json_string(address)
        );
        let reply = self.call_listener("bind", payload.as_bytes(), "MAIL4011")?;
        let text = reply_text(&reply, "bind")?;
        Ok(Listener {
            handle: string_field(text, "handle")?,
            local: string_field(text, "local")?,
        })
    }

    pub fn accept(&self, listener: &str, timeout_ms: Option<u64>) -> Result<Connection, String> {
        require_handle(listener, "listener")?;
        let payload = format!(
            "{{\"handle\":{},\"timeout_ms\":{}}}",
            json_string(listener),
            option_u64(timeout_ms)
        );
        let reply = self.call_listener("accept", payload.as_bytes(), "MAIL4011")?;
        let text = reply_text(&reply, "accept")?;
        Ok(Connection {
            handle: string_field(text, "handle")?,
            peer: string_field(text, "peer")?,
            local: string_field(text, "local")?,
        })
    }

    pub fn read(
        &self,
        connection: &str,
        max_bytes: usize,
        timeout_ms: Option<u64>,
    ) -> Result<ReadResult, String> {
        require_handle(connection, "connection")?;
        if max_bytes == 0 || max_bytes > MAX_READ_BYTES {
            return Err(format!(
                "MAIL1002: SMTP read max_bytes must be in 1..={MAX_READ_BYTES}"
            ));
        }
        let payload = format!(
            "{{\"handle\":{},\"timeout_ms\":{},\"data\":[],\"max_bytes\":{max_bytes}}}",
            json_string(connection),
            option_u64(timeout_ms)
        );
        let reply = self.call_listener("read", payload.as_bytes(), "MAIL4007")?;
        let text = reply_text(&reply, "read")?;
        Ok(ReadResult {
            data: byte_array_field(text, "data")?,
            eof: bool_field(text, "eof")?,
            tls: bool_field(text, "tls").unwrap_or(false),
        })
    }

    pub fn write(
        &self,
        connection: &str,
        data: &[u8],
        timeout_ms: Option<u64>,
    ) -> Result<usize, String> {
        require_handle(connection, "connection")?;
        if data.is_empty() || data.len() > MAX_WRITE_BYTES {
            return Err(format!(
                "MAIL1002: SMTP write data must contain 1..={MAX_WRITE_BYTES} bytes"
            ));
        }
        let payload = format!(
            "{{\"handle\":{},\"timeout_ms\":{},\"data\":{},\"max_bytes\":0}}",
            json_string(connection),
            option_u64(timeout_ms),
            json_bytes(data)
        );
        let reply = self.call_listener("write", payload.as_bytes(), "MAIL4007")?;
        let text = reply_text(&reply, "write")?;
        usize_field(text, "written")
    }

    pub fn start_tls(
        &self,
        connection: &str,
        cert_chain_der: &[Vec<u8>],
        private_key_pkcs8_der: &[u8],
        timeout_ms: Option<u64>,
    ) -> Result<TlsResult, String> {
        require_handle(connection, "connection")?;
        if cert_chain_der.is_empty() || private_key_pkcs8_der.is_empty() {
            return Err("MAIL4010: inbound STARTTLS requires a non-empty DER certificate chain and PKCS#8 private key".into());
        }
        let certificates = cert_chain_der
            .iter()
            .map(|certificate| json_bytes(certificate))
            .collect::<Vec<_>>()
            .join(",");
        let payload = format!(
            "{{\"handle\":{},\"cert_chain_der\":[{certificates}],\"private_key_pkcs8_der\":{},\"timeout_ms\":{}}}",
            json_string(connection),
            json_bytes(private_key_pkcs8_der),
            option_u64(timeout_ms)
        );
        let reply = self.call_listener("start_tls", payload.as_bytes(), "MAIL4010")?;
        let text = reply_text(&reply, "STARTTLS")?;
        Ok(TlsResult {
            handle: string_field(text, "handle")?,
            tls: bool_field(text, "tls")?,
        })
    }

    pub fn close_connection(&self, connection: &str) -> Result<(), String> {
        require_handle(connection, "connection")?;
        let payload = format!("{{\"handle\":{}}}", json_string(connection));
        self.call_listener("close_connection", payload.as_bytes(), "MAIL4007")?;
        Ok(())
    }

    pub fn close(&self, listener: &str) -> Result<(), String> {
        require_handle(listener, "listener")?;
        let payload = format!("{{\"handle\":{}}}", json_string(listener));
        self.call_listener("close", payload.as_bytes(), "MAIL4011")?;
        Ok(())
    }

    /// Generate the 256-bit ownership token used by `_rbe-mail.<domain>`.
    pub fn ownership_token(&self) -> Result<String, String> {
        self.require_crypto()?;
        let reply = self
            .bridge
            .call(HostCall::new(
                CRYPTO_CAPABILITY,
                CRYPTO_CAPABILITY,
                "random",
                br#"{"bytes":32}"#,
            ))
            .map_err(|error| format!("MAIL2007: RBE secure random generation failed: {error}"))?;
        let text = reply_text(&reply, "secure random")?;
        let token = string_field(text, "data_hex")?;
        if token.len() != 64 || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("MAIL8001: RBE crypto random response did not contain a 256-bit hexadecimal token".into());
        }
        Ok(token)
    }

    pub fn ed25519_public_key(&self, seed_hex: &str) -> Result<String, String> {
        self.require_crypto()?;
        require_ed25519_seed(seed_hex)?;
        let payload = format!("{{\"seed_hex\":{}}}", json_string(seed_hex));
        let reply = self
            .bridge
            .call(HostCall::new(
                CRYPTO_CAPABILITY,
                CRYPTO_CAPABILITY,
                "ed25519_public",
                payload.as_bytes(),
            ))
            .map_err(|error| format!("MAIL4007: RBE Ed25519 public-key derivation failed: {error}"))?;
        string_field(reply_text(&reply, "Ed25519 public key")?, "public_key_hex")
    }

    pub fn ed25519_sign(&self, seed_hex: &str, data: &[u8]) -> Result<Ed25519Signature, String> {
        self.require_crypto()?;
        require_ed25519_seed(seed_hex)?;
        if data.is_empty() {
            return Err("MAIL1002: Ed25519 signing input cannot be empty".into());
        }
        let payload = format!(
            "{{\"seed_hex\":{},\"data_hex\":{}}}",
            json_string(seed_hex),
            json_string(&hex_encode(data))
        );
        let reply = self
            .bridge
            .call(HostCall::new(
                CRYPTO_CAPABILITY,
                CRYPTO_CAPABILITY,
                "ed25519_sign",
                payload.as_bytes(),
            ))
            .map_err(|error| format!("MAIL4007: RBE Ed25519 signing failed: {error}"))?;
        let text = reply_text(&reply, "Ed25519 signature")?;
        Ok(Ed25519Signature {
            public_key_hex: string_field(text, "public_key_hex")?,
            signature_hex: string_field(text, "signature_hex")?,
        })
    }

    fn require_crypto(&self) -> Result<(), String> {
        let sdk = RbeSdk::new(self.bridge);
        if sdk.host().granted(CRYPTO_CAPABILITY) == Some(true) {
            Ok(())
        } else {
            Err("MAIL2001: RBE crypto capability is not granted; approve crypto for secure mail token/signature operations".into())
        }
    }

    fn call_listener(
        &self,
        operation: &str,
        payload: &[u8],
        code: &str,
    ) -> Result<HostReply, String> {
        self.bridge
            .call(HostCall::new(
                LISTENER_CAPABILITY,
                LISTENER_CAPABILITY,
                operation,
                payload,
            ))
            .map_err(|error| format!("{code}: RBE inbound SMTP {operation} operation failed: {error}"))
    }
}

fn require_handle(handle: &str, label: &str) -> Result<(), String> {
    if handle.is_empty() || handle.len() > 256 || handle.chars().any(char::is_control) {
        Err(format!("MAIL1002: SMTP {label} handle is invalid"))
    } else {
        Ok(())
    }
}

fn require_ed25519_seed(seed_hex: &str) -> Result<(), String> {
    if seed_hex.len() == 64 && seed_hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err("MAIL1002: Ed25519 private seed must contain exactly 64 hexadecimal characters (32 bytes)".into())
    }
}

fn option_u64(value: Option<u64>) -> String {
    value.map_or_else(|| "null".into(), |value| value.to_string())
}

fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                use std::fmt::Write as _;
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn json_bytes(bytes: &[u8]) -> String {
    let mut out = String::from("[");
    for (index, byte) in bytes.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&byte.to_string());
    }
    out.push(']');
    out
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn reply_text<'a>(reply: &'a HostReply, operation: &str) -> Result<&'a str, String> {
    std::str::from_utf8(&reply.payload)
        .map_err(|_| format!("MAIL8001: RBE {operation} response was not UTF-8 JSON"))
}

fn field_tail<'a>(input: &'a str, field: &str) -> Result<&'a str, String> {
    let needle = format!("\"{field}\"");
    let start = input
        .find(&needle)
        .ok_or_else(|| format!("MAIL8001: RBE response is missing field {field:?}"))?
        + needle.len();
    let tail = &input[start..];
    let colon = tail
        .find(':')
        .ok_or_else(|| format!("MAIL8001: RBE response field {field:?} has no value"))?;
    Ok(tail[colon + 1..].trim_start())
}

fn string_field(input: &str, field: &str) -> Result<String, String> {
    let mut chars = field_tail(input, field)?.chars();
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

fn bool_field(input: &str, field: &str) -> Result<bool, String> {
    let tail = field_tail(input, field)?;
    if tail.starts_with("true") {
        Ok(true)
    } else if tail.starts_with("false") {
        Ok(false)
    } else {
        Err(format!("MAIL8001: RBE response field {field:?} is not boolean"))
    }
}

fn usize_field(input: &str, field: &str) -> Result<usize, String> {
    let tail = field_tail(input, field)?;
    let end = tail
        .find(|ch: char| !ch.is_ascii_digit())
        .unwrap_or(tail.len());
    if end == 0 {
        return Err(format!("MAIL8001: RBE response field {field:?} is not an integer"));
    }
    tail[..end]
        .parse()
        .map_err(|_| format!("MAIL8001: RBE response field {field:?} is outside usize range"))
}

fn byte_array_field(input: &str, field: &str) -> Result<Vec<u8>, String> {
    let tail = field_tail(input, field)?;
    let Some(rest) = tail.strip_prefix('[') else {
        return Err(format!("MAIL8001: RBE response field {field:?} is not a byte array"));
    };
    let end = rest
        .find(']')
        .ok_or_else(|| format!("MAIL8001: RBE response field {field:?} has an unterminated byte array"))?;
    let body = rest[..end].trim();
    if body.is_empty() {
        return Ok(Vec::new());
    }
    body.split(',')
        .map(|part| {
            part.trim()
                .parse::<u8>()
                .map_err(|_| format!("MAIL8001: RBE response field {field:?} contains a non-byte value"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bounded_host_envelopes() {
        let read = r#"{"data":[83,77,84,80],"eof":false,"tls":true}"#;
        assert_eq!(byte_array_field(read, "data").unwrap(), b"SMTP");
        assert!(!bool_field(read, "eof").unwrap());
        assert!(bool_field(read, "tls").unwrap());
        assert_eq!(usize_field(r#"{"written":42}"#, "written").unwrap(), 42);
    }

    #[test]
    fn JSON_byte_encoding_is_exact() {
        assert_eq!(json_bytes(&[0, 1, 127, 255]), "[0,1,127,255]");
        assert_eq!(hex_encode(&[0xab, 0xcd]), "abcd");
    }
}
