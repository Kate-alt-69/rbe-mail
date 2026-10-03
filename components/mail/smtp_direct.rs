use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use rbe_sdk::{HostCall, RbeSdk};

use crate::json;
use crate::self_hosted;
use crate::types::{
    error_codes, Address, DeliveryStatus, MailError, MailErrorKind, Message, ProviderKind,
    SendReceipt,
};

const TLS_CAPABILITY: &str = "net:tls";
const DEFAULT_TIMEOUT_MS: u64 = 8_000;
const MAX_REPLY_BYTES: usize = 64 * 1024;
const MAX_READ_BYTES: usize = 4 * 1024;
const MAX_MESSAGE_BYTES: usize = 8 * 1024 * 1024;
const MAX_WRITE_CHUNK: usize = 48 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
struct SmtpReply {
    code: u16,
    text: String,
}

pub fn send(
    sdk: RbeSdk<'_>,
    local_domain: &str,
    from: &Address,
    to: &[Address],
    message: &Message,
) -> Result<SendReceipt, MailError> {
    self_hosted::require_secure_outbound(sdk)?;
    require_ascii_mailbox(from)?;
    for address in to.iter().chain(&message.cc).chain(&message.bcc) {
        require_ascii_mailbox(address)?;
    }
    if let Some(reply_to) = &message.reply_to {
        require_ascii_mailbox(reply_to)?;
    }

    let mut by_domain: BTreeMap<String, Vec<Address>> = BTreeMap::new();
    for address in to.iter().chain(&message.cc).chain(&message.bcc) {
        let domain = address
            .email
            .rsplit_once('@')
            .map(|(_, domain)| domain.trim_end_matches('.').to_ascii_lowercase())
            .ok_or_else(|| MailError::invalid_message("recipient is missing a domain"))?;
        by_domain.entry(domain).or_default().push(address.clone());
    }

    let message_id = generate_message_id(sdk, local_domain).ok();
    let encoded = build_message(from, to, message, message_id.as_deref())?;
    let mut delivered_domains = 0usize;

    for (recipient_domain, recipients) in by_domain {
        match send_domain(
            sdk,
            local_domain,
            from,
            &recipient_domain,
            &recipients,
            &encoded,
        ) {
            Ok(()) => delivered_domains += 1,
            Err(error) if delivered_domains > 0 => {
                return Err(MailError::new(
                    error_codes::SMTP_PARTIAL_DELIVERY,
                    MailErrorKind::Network,
                    false,
                    format!(
                        "self-hosted SMTP delivered to {delivered_domains} recipient domain(s) before {recipient_domain} failed with {} {}; do not blindly resend the full message",
                        error.code, error.message
                    ),
                ));
            }
            Err(error) => return Err(error),
        }
    }

    Ok(SendReceipt {
        provider: ProviderKind::SelfHosted,
        message_id,
        status: DeliveryStatus::Accepted,
    })
}

fn send_domain(
    sdk: RbeSdk<'_>,
    local_domain: &str,
    from: &Address,
    recipient_domain: &str,
    recipients: &[Address],
    message: &[u8],
) -> Result<(), MailError> {
    let mx = self_hosted::resolve_mx(sdk, recipient_domain)?;
    let mut last_error = None;

    for record in mx {
        let host = record.exchange.trim_end_matches('.');
        match smtp_transaction(sdk, local_domain, host, from, recipients, message) {
            Ok(()) => return Ok(()),
            Err(error)
                if error.retryable
                    || matches!(
                        error.code,
                        error_codes::NETWORK | error_codes::SMTP_TLS_UNAVAILABLE
                    ) =>
            {
                last_error = Some(error);
            }
            Err(error) => return Err(error),
        }
    }

    Err(last_error.unwrap_or_else(|| {
        MailError::network(format!(
            "no usable SMTP exchanger accepted a connection for {recipient_domain}"
        ))
    }))
}

fn smtp_transaction(
    sdk: RbeSdk<'_>,
    local_domain: &str,
    mx_host: &str,
    from: &Address,
    recipients: &[Address],
    message: &[u8],
) -> Result<(), MailError> {
    let handle = connect(sdk, mx_host, 25)?;
    let result = (|| {
        expect_code(read_reply(sdk, &handle)?, &[220], "SMTP greeting")?;

        let ehlo = command(sdk, &handle, &format!("EHLO {local_domain}\r\n"))?;
        expect_code(ehlo.clone(), &[250], "EHLO")?;
        if !advertises(&ehlo.text, "STARTTLS") {
            return Err(MailError::unsupported(
                error_codes::SMTP_TLS_UNAVAILABLE,
                format!("SMTP exchanger {mx_host} did not advertise STARTTLS"),
            ));
        }

        expect_code(
            command(sdk, &handle, "STARTTLS\r\n")?,
            &[220],
            "STARTTLS command",
        )?;
        start_tls(sdk, &handle)?;

        expect_code(
            command(sdk, &handle, &format!("EHLO {local_domain}\r\n"))?,
            &[250],
            "post-TLS EHLO",
        )?;

        expect_code(
            command(
                sdk,
                &handle,
                &format!("MAIL FROM:<{}>\r\n", from.email.trim()),
            )?,
            &[250],
            "MAIL FROM",
        )?;

        for recipient in recipients {
            let reply = command(
                sdk,
                &handle,
                &format!("RCPT TO:<{}>\r\n", recipient.email.trim()),
            )?;
            expect_code(reply, &[250, 251], "RCPT TO")?;
        }

        expect_code(command(sdk, &handle, "DATA\r\n")?, &[354], "DATA")?;
        let mut stuffed = dot_stuff(message);
        if !stuffed.ends_with(b"\r\n") {
            stuffed.extend_from_slice(b"\r\n");
        }
        stuffed.extend_from_slice(b".\r\n");
        write_all(sdk, &handle, &stuffed)?;
        expect_code(read_reply(sdk, &handle)?, &[250], "message body")?;

        // The message is already accepted after the final 250. QUIT is best effort:
        // a broken QUIT response must never turn an accepted message into a retry.
        let _ = command(sdk, &handle, "QUIT\r\n");
        Ok(())
    })();

    let _ = close(sdk, &handle);
    result
}

fn connect(sdk: RbeSdk<'_>, host: &str, port: u16) -> Result<String, MailError> {
    let payload = format!(
        "{{\"host\":{},\"port\":{port},\"timeout_ms\":{DEFAULT_TIMEOUT_MS}}}",
        json::string(host)
    );
    let reply = sdk
        .call(HostCall::new(
            TLS_CAPABILITY,
            TLS_CAPABILITY,
            "connect",
            payload.as_bytes(),
        ))
        .map_err(|error| {
            MailError::network(format!("RBE net:tls connect to {host}:{port} failed: {error}"))
        })?;
    let text = std::str::from_utf8(&reply.payload)
        .map_err(|_| malformed("RBE net:tls connect response was not UTF-8 JSON"))?;
    json::find_string_field(text, "handle")
        .ok_or_else(|| malformed("RBE net:tls connect response is missing handle"))
}

fn start_tls(sdk: RbeSdk<'_>, handle: &str) -> Result<(), MailError> {
    let payload = format!(
        "{{\"handle\":{},\"timeout_ms\":{DEFAULT_TIMEOUT_MS}}}",
        json::string(handle)
    );
    let reply = sdk
        .call(HostCall::new(
            TLS_CAPABILITY,
            TLS_CAPABILITY,
            "start_tls",
            payload.as_bytes(),
        ))
        .map_err(|error| {
            MailError::unsupported(
                error_codes::SMTP_TLS_UNAVAILABLE,
                format!(
                    "RBE STARTTLS handshake failed and the connection was destroyed to prevent plaintext fallback: {error}"
                ),
            )
        })?;
    let text = std::str::from_utf8(&reply.payload)
        .map_err(|_| malformed("RBE STARTTLS response was not UTF-8 JSON"))?;
    if json::find_bool_field(text, "tls") != Some(true) {
        return Err(malformed(
            "RBE STARTTLS response did not confirm encrypted transport",
        ));
    }
    Ok(())
}

fn close(sdk: RbeSdk<'_>, handle: &str) -> Result<(), MailError> {
    let payload = format!("{{\"handle\":{}}}", json::string(handle));
    sdk.call(HostCall::new(
        TLS_CAPABILITY,
        TLS_CAPABILITY,
        "close",
        payload.as_bytes(),
    ))
    .map(|_| ())
    .map_err(|error| MailError::network(format!("RBE net:tls close failed: {error}")))
}

fn command(sdk: RbeSdk<'_>, handle: &str, command: &str) -> Result<SmtpReply, MailError> {
    write_all(sdk, handle, command.as_bytes())?;
    read_reply(sdk, handle)
}

fn write_all(sdk: RbeSdk<'_>, handle: &str, data: &[u8]) -> Result<(), MailError> {
    for chunk in data.chunks(MAX_WRITE_CHUNK) {
        let payload = format!(
            "{{\"handle\":{},\"data\":{},\"timeout_ms\":{DEFAULT_TIMEOUT_MS}}}",
            json::string(handle),
            json_bytes(chunk)
        );
        let reply = sdk
            .call(HostCall::new(
                TLS_CAPABILITY,
                TLS_CAPABILITY,
                "write",
                payload.as_bytes(),
            ))
            .map_err(|error| MailError::network(format!("RBE net:tls write failed: {error}")))?;
        let text = std::str::from_utf8(&reply.payload)
            .map_err(|_| malformed("RBE net:tls write response was not UTF-8 JSON"))?;
        let written = json::find_u16_field(text, "written")
            .map(usize::from)
            .or_else(|| find_usize_field(text, "written"))
            .ok_or_else(|| malformed("RBE net:tls write response is missing written byte count"))?;
        if written != chunk.len() {
            return Err(MailError::network(format!(
                "RBE net:tls short write: expected {} bytes, wrote {written}",
                chunk.len()
            )));
        }
    }
    Ok(())
}

fn read_reply(sdk: RbeSdk<'_>, handle: &str) -> Result<SmtpReply, MailError> {
    let mut buffer = Vec::new();
    for _ in 0..32 {
        let payload = format!(
            "{{\"handle\":{},\"max_bytes\":{MAX_READ_BYTES},\"timeout_ms\":{DEFAULT_TIMEOUT_MS}}}",
            json::string(handle)
        );
        let reply = sdk
            .call(HostCall::new(
                TLS_CAPABILITY,
                TLS_CAPABILITY,
                "read",
                payload.as_bytes(),
            ))
            .map_err(|error| MailError::network(format!("RBE net:tls read failed: {error}")))?;
        let text = std::str::from_utf8(&reply.payload)
            .map_err(|_| malformed("RBE net:tls read response was not UTF-8 JSON"))?;
        let data = byte_array_field(text, "data")?;
        let eof = json::find_bool_field(text, "eof")
            .ok_or_else(|| malformed("RBE net:tls read response is missing eof"))?;
        buffer.extend_from_slice(&data);
        if buffer.len() > MAX_REPLY_BYTES {
            return Err(MailError::new(
                error_codes::RBE_LIMIT,
                MailErrorKind::RbeLimit,
                false,
                format!("SMTP reply exceeded {MAX_REPLY_BYTES} bytes"),
            ));
        }
        if let Some(reply) = parse_complete_reply(&buffer)? {
            return Ok(reply);
        }
        if eof {
            return Err(malformed(
                "SMTP peer closed before a complete reply was received",
            ));
        }
    }
    Err(MailError::new(
        error_codes::SMTP_TEMPORARY_FAILURE,
        MailErrorKind::Timeout,
        true,
        "SMTP reply did not complete within the bounded read budget",
    ))
}

fn parse_complete_reply(buffer: &[u8]) -> Result<Option<SmtpReply>, MailError> {
    let text = std::str::from_utf8(buffer)
        .map_err(|_| malformed("SMTP reply was not valid UTF-8/ASCII"))?;
    let mut expected = None;
    let mut consumed = 0usize;

    for line in text.split_inclusive("\r\n") {
        if !line.ends_with("\r\n") {
            break;
        }
        consumed += line.len();
        let content = &line[..line.len() - 2];
        if content.len() < 3 || !content.as_bytes()[..3].iter().all(u8::is_ascii_digit) {
            return Err(malformed(
                "SMTP reply line does not begin with a three-digit status code",
            ));
        }
        let code: u16 = content[..3]
            .parse()
            .map_err(|_| malformed("SMTP status code is malformed"))?;
        if let Some(previous) = expected {
            if previous != code {
                return Err(malformed(
                    "SMTP multiline reply changed status code before completion",
                ));
            }
        } else {
            expected = Some(code);
        }

        let final_line = content.len() == 3 || content.as_bytes().get(3) == Some(&b' ');
        let continued = content.as_bytes().get(3) == Some(&b'-');
        if !final_line && !continued {
            return Err(malformed(
                "SMTP reply status code must be followed by space or hyphen",
            ));
        }
        if final_line {
            return Ok(Some(SmtpReply {
                code,
                text: text[..consumed].to_string(),
            }));
        }
    }
    Ok(None)
}

fn expect_code(reply: SmtpReply, accepted: &[u16], stage: &str) -> Result<SmtpReply, MailError> {
    if accepted.contains(&reply.code) {
        return Ok(reply);
    }
    if (400..500).contains(&reply.code) {
        return Err(MailError::new(
            error_codes::SMTP_TEMPORARY_FAILURE,
            MailErrorKind::ProviderUnavailable,
            true,
            format!(
                "{stage} received temporary SMTP status {}: {}",
                reply.code,
                sanitize_reply(&reply.text)
            ),
        ));
    }
    if (500..600).contains(&reply.code) {
        return Err(MailError::new(
            error_codes::SMTP_PERMANENT_REJECTION,
            MailErrorKind::RecipientRejected,
            false,
            format!(
                "{stage} received permanent SMTP status {}: {}",
                reply.code,
                sanitize_reply(&reply.text)
            ),
        ));
    }
    Err(malformed(format!(
        "{stage} received unexpected SMTP status {}: {}",
        reply.code,
        sanitize_reply(&reply.text)
    )))
}

fn advertises(reply: &str, capability: &str) -> bool {
    reply.lines().any(|line| {
        let tail = line.get(4..).unwrap_or_default();
        tail.split_whitespace()
            .next()
            .is_some_and(|token| token.eq_ignore_ascii_case(capability))
    })
}

fn build_message(
    from: &Address,
    to: &[Address],
    message: &Message,
    message_id: Option<&str>,
) -> Result<Vec<u8>, MailError> {
    let to_header = to
        .iter()
        .map(|address| address.email.trim())
        .collect::<Vec<_>>()
        .join(", ");
    let cc_header = message
        .cc
        .iter()
        .map(|address| address.email.trim())
        .collect::<Vec<_>>()
        .join(", ");
    let subject = encoded_word(&message.subject);

    let mut out = String::new();
    out.push_str(&format!("Date: {}\r\n", date_header()?));
    out.push_str(&format!("From: <{}>\r\n", from.email.trim()));
    out.push_str(&format!("To: {to_header}\r\n"));
    if !cc_header.is_empty() {
        out.push_str(&format!("Cc: {cc_header}\r\n"));
    }
    if let Some(reply_to) = &message.reply_to {
        out.push_str(&format!("Reply-To: <{}>\r\n", reply_to.email.trim()));
    }
    if let Some(message_id) = message_id {
        out.push_str(&format!("Message-ID: {message_id}\r\n"));
    }
    out.push_str(&format!("Subject: {subject}\r\n"));
    out.push_str("MIME-Version: 1.0\r\n");

    match (&message.text, &message.html) {
        (Some(text), Some(html)) => {
            let boundary = choose_boundary(text, html);
            out.push_str(&format!(
                "Content-Type: multipart/alternative; boundary=\"{boundary}\"\r\n\r\n"
            ));
            append_part(&mut out, &boundary, "text/plain", text);
            append_part(&mut out, &boundary, "text/html", html);
            out.push_str(&format!("--{boundary}--\r\n"));
        }
        (Some(text), None) => append_single(&mut out, "text/plain", text),
        (None, Some(html)) => append_single(&mut out, "text/html", html),
        (None, None) => return Err(MailError::invalid_message("email needs text or html content")),
    }

    if out.len() > MAX_MESSAGE_BYTES {
        return Err(MailError::new(
            error_codes::RBE_LIMIT,
            MailErrorKind::RbeLimit,
            false,
            format!("self-hosted SMTP message exceeds {MAX_MESSAGE_BYTES} bytes"),
        ));
    }
    Ok(out.into_bytes())
}

fn date_header() -> Result<String, MailError> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| {
            MailError::new(
                "MAIL9002",
                MailErrorKind::Internal,
                false,
                "system clock is before UNIX epoch; cannot create RFC 5322 Date header",
            )
        })?
        .as_secs() as i64;
    let days = seconds.div_euclid(86_400);
    let remainder = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = remainder / 3_600;
    let minute = (remainder % 3_600) / 60;
    let second = remainder % 60;
    let weekdays = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    let months = [
        "", "Jan", "Feb", "Mar", "Apr", "May", "Jun",
        "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let weekday = weekdays[(days + 4).rem_euclid(7) as usize];
    let month_name = months
        .get(month as usize)
        .copied()
        .ok_or_else(|| MailError::invariant("RFC 5322 date month is outside 1..=12"))?;
    Ok(format!(
        "{weekday}, {day:02} {month_name} {year:04} {hour:02}:{minute:02}:{second:02} +0000"
    ))
}

fn civil_from_days(days_since_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u32;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe as i32 + era as i32 * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp as i32 + if mp < 10 { 3 } else { -9 };
    (
        year + if month <= 2 { 1 } else { 0 },
        month as u32,
        day,
    )
}

fn append_single(out: &mut String, content_type: &str, body: &str) {
    out.push_str(&format!(
        "Content-Type: {content_type}; charset=utf-8\r\nContent-Transfer-Encoding: base64\r\n\r\n"
    ));
    out.push_str(&base64_wrapped(body.as_bytes()));
}

fn append_part(out: &mut String, boundary: &str, content_type: &str, body: &str) {
    out.push_str(&format!(
        "--{boundary}\r\nContent-Type: {content_type}; charset=utf-8\r\nContent-Transfer-Encoding: base64\r\n\r\n"
    ));
    out.push_str(&base64_wrapped(body.as_bytes()));
}

fn choose_boundary(text: &str, html: &str) -> String {
    let mut suffix = fingerprint(text.as_bytes()) ^ fingerprint(html.as_bytes());
    loop {
        let candidate = format!("rbe-mail-{suffix:016x}");
        if !text.contains(&candidate) && !html.contains(&candidate) {
            return candidate;
        }
        suffix = suffix.wrapping_add(1);
    }
}

fn fingerprint(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn encoded_word(value: &str) -> String {
    if value.is_ascii() {
        value.to_string()
    } else {
        format!("=?UTF-8?B?{}?=", base64(value.as_bytes()))
    }
}

fn base64_wrapped(bytes: &[u8]) -> String {
    let encoded = base64(bytes);
    let mut out = String::with_capacity(encoded.len() + encoded.len() / 76 * 2 + 2);
    for chunk in encoded.as_bytes().chunks(76) {
        out.push_str(std::str::from_utf8(chunk).expect("base64 is ASCII"));
        out.push_str("\r\n");
    }
    out
}

fn base64(input: &[u8]) -> String {
    const TABLE: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(((input.len() + 2) / 3) * 4);
    for chunk in input.chunks(3) {
        let a = chunk[0];
        let b = *chunk.get(1).unwrap_or(&0);
        let c = *chunk.get(2).unwrap_or(&0);
        out.push(TABLE[(a >> 2) as usize] as char);
        out.push(TABLE[(((a & 3) << 4) | (b >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[(((b & 15) << 2) | (c >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(c & 63) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

fn dot_stuff(message: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(message.len() + 32);
    let mut line_start = true;
    for byte in message {
        if line_start && *byte == b'.' {
            out.push(b'.');
        }
        out.push(*byte);
        if *byte == b'\n' {
            line_start = true;
        } else if *byte != b'\r' {
            line_start = false;
        }
    }
    out
}

fn generate_message_id(sdk: RbeSdk<'_>, domain: &str) -> Result<String, MailError> {
    if sdk.host().granted("crypto") != Some(true) {
        return Err(MailError::capability("crypto"));
    }
    let reply = sdk
        .call(HostCall::new(
            "crypto",
            "crypto",
            "random",
            br#"{"bytes":16}"#,
        ))
        .map_err(|error| MailError::network(format!("RBE crypto random failed: {error}")))?;
    let text = std::str::from_utf8(&reply.payload)
        .map_err(|_| malformed("RBE crypto response was not UTF-8 JSON"))?;
    let random = json::find_string_field(text, "data_hex")
        .ok_or_else(|| malformed("RBE crypto response is missing data_hex"))?;
    if random.len() != 32 || !random.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(malformed(
            "RBE crypto response did not contain 16 random bytes",
        ));
    }
    Ok(format!("<{}@{}>", random.to_ascii_lowercase(), domain.trim_end_matches('.')))
}

fn require_ascii_mailbox(address: &Address) -> Result<(), MailError> {
    if address.email.is_ascii() {
        Ok(())
    } else {
        Err(MailError::new(
            error_codes::SMTPUTF8_UNSUPPORTED,
            MailErrorKind::Unsupported,
            false,
            format!(
                "self-hosted SMTP currently requires ASCII envelope addresses; SMTPUTF8 negotiation is not implemented for {:?}",
                address.email
            ),
        ))
    }
}

fn malformed(message: impl Into<String>) -> MailError {
    MailError::new(
        error_codes::MALFORMED_SMTP_RESPONSE,
        MailErrorKind::ExternalResponse,
        false,
        message,
    )
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

fn byte_array_field(input: &str, field: &str) -> Result<Vec<u8>, MailError> {
    let pattern = format!("\"{field}\"");
    let start = input
        .find(&pattern)
        .ok_or_else(|| malformed(format!("RBE response is missing field {field:?}")))?;
    let tail = &input[start + pattern.len()..];
    let colon = tail
        .find(':')
        .ok_or_else(|| malformed(format!("RBE response field {field:?} has no value")))?;
    let tail = tail[colon + 1..].trim_start();
    let body = tail
        .strip_prefix('[')
        .and_then(|tail| tail.split_once(']').map(|(body, _)| body))
        .ok_or_else(|| malformed(format!("RBE response field {field:?} is not a byte array")))?;
    if body.trim().is_empty() {
        return Ok(Vec::new());
    }
    body.split(',')
        .map(|part| {
            part.trim()
                .parse::<u8>()
                .map_err(|_| malformed(format!("RBE response field {field:?} contains a non-byte value")))
        })
        .collect()
}

fn find_usize_field(input: &str, field: &str) -> Option<usize> {
    let pattern = format!("\"{field}\"");
    let start = input.find(&pattern)? + pattern.len();
    let tail = &input[start..];
    let colon = tail.find(':')?;
    let tail = tail[colon + 1..].trim_start();
    let end = tail
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(tail.len());
    (end > 0).then(|| tail[..end].parse().ok()).flatten()
}

fn sanitize_reply(reply: &str) -> String {
    let mut out = reply.replace('\r', " ").replace('\n', " ");
    if out.len() > 512 {
        out.truncate(512);
        out.push_str("...");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_multiline_smtp_reply() {
        let reply = parse_complete_reply(b"250-mail.example\r\n250-PIPELINING\r\n250 STARTTLS\r\n")
            .unwrap()
            .unwrap();
        assert_eq!(reply.code, 250);
        assert!(advertises(&reply.text, "STARTTLS"));
    }

    #[test]
    fn waits_for_final_multiline_line() {
        assert!(parse_complete_reply(b"250-mail.example\r\n250-PIPELINING\r\n")
            .unwrap()
            .is_none());
    }

    #[test]
    fn dot_stuffs_every_line_start() {
        assert_eq!(
            dot_stuff(b".one\r\n..two\r\nthree\r\n"),
            b"..one\r\n...two\r\nthree\r\n"
        );
    }

    #[test]
    fn base64_is_rfc4648_compatible() {
        assert_eq!(base64(b"hello"), "aGVsbG8=");
    }
}
