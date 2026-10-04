use rbe_sdk::{HostBridge, HostCall, HostReply, RbeSdk};

const STORAGE_CAPABILITY: &str = "storage";
const CRYPTO_CAPABILITY: &str = "crypto";
const CHUNK_BYTES: usize = 192 * 1024;
const MAX_MESSAGE_BYTES: usize = 32 * 1024 * 1024;
const MAX_METADATA_BYTES: usize = 64 * 1024;
const MAX_LIST_RESULTS: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MailFolder {
    Inbox,
    Sent,
    Queue,
    Failed,
}

impl MailFolder {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Inbox => "inbox",
            Self::Sent => "sent",
            Self::Queue => "queue",
            Self::Failed => "failed",
        }
    }

    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "inbox" => Ok(Self::Inbox),
            "sent" => Ok(Self::Sent),
            "queue" => Ok(Self::Queue),
            "failed" => Ok(Self::Failed),
            _ => Err("MAIL4021: stored mail manifest contains an unknown folder".into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredMail {
    pub id: String,
    pub folder: MailFolder,
    pub raw: Vec<u8>,
    pub metadata_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredMailSummary {
    pub id: String,
    pub folder: MailFolder,
    pub bytes: usize,
    pub chunks: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Manifest {
    id: String,
    folder: MailFolder,
    bytes: usize,
    chunks: usize,
    chunk_sha256: Vec<String>,
}

pub struct MailStore<'a> {
    bridge: &'a dyn HostBridge,
}

impl<'a> MailStore<'a> {
    pub fn new(bridge: &'a dyn HostBridge) -> Result<Self, String> {
        let sdk = RbeSdk::new(bridge);
        let Some(session) = sdk.host().session() else {
            return Err("MAIL2006: RBE Library Host session is unavailable; mail-store requires an accepted package host session".into());
        };
        if !session.granted(STORAGE_CAPABILITY) {
            return Err("MAIL4020: RBE storage capability is not granted; approve durable package storage before enabling inbox/queue persistence".into());
        }
        if !session.granted(CRYPTO_CAPABILITY) {
            return Err("MAIL2001: RBE crypto capability is not granted; mail-store requires SHA-256 verification for persisted chunks".into());
        }
        Ok(Self { bridge })
    }

    /// Persist one immutable raw RFC5322 message.
    ///
    /// Chunks and metadata are written first. `manifest.json` is the commit
    /// record and is written last. Folder indexing happens after commit; an
    /// index failure therefore never turns partial bytes into a valid message.
    pub fn put(
        &self,
        id: &str,
        folder: MailFolder,
        raw: &[u8],
        metadata_json: &str,
    ) -> Result<StoredMailSummary, String> {
        validate_id(id)?;
        if raw.is_empty() || raw.len() > MAX_MESSAGE_BYTES {
            return Err(format!(
                "MAIL1002: raw message must contain 1..={MAX_MESSAGE_BYTES} bytes"
            ));
        }
        if metadata_json.len() > MAX_METADATA_BYTES {
            return Err(format!(
                "MAIL1002: message metadata must be at most {MAX_METADATA_BYTES} UTF-8 bytes"
            ));
        }

        let manifest_key = manifest_key(id);
        if self.exists(&manifest_key)? {
            return Err(format!(
                "MAIL4023: message id {id:?} is already committed; generate a unique immutable storage id"
            ));
        }

        let mut digests = Vec::new();
        for (index, chunk) in raw.chunks(CHUNK_BYTES).enumerate() {
            let digest = self.sha256(chunk)?;
            self.put_object(&chunk_key(id, index), chunk)?;
            digests.push(digest);
        }

        self.put_object(&metadata_key(id), metadata_json.as_bytes())?;
        let manifest = Manifest {
            id: id.to_string(),
            folder,
            bytes: raw.len(),
            chunks: digests.len(),
            chunk_sha256: digests,
        };
        self.put_object(&manifest_key, encode_manifest(&manifest).as_bytes())?;

        if let Err(error) = self.put_object(&index_key(folder, id), id.as_bytes()) {
            return Err(format!(
                "MAIL4022: message {id:?} is committed but its {} index pointer could not be written: {error}; call repair_index() instead of storing or delivering the message again",
                folder.as_str()
            ));
        }

        Ok(summary_from_manifest(&manifest))
    }

    pub fn get(&self, id: &str) -> Result<Option<StoredMail>, String> {
        validate_id(id)?;
        let Some(manifest_bytes) = self.get_object(&manifest_key(id))? else {
            return Ok(None);
        };
        let manifest = decode_manifest(id, &manifest_bytes)?;

        let mut raw = Vec::with_capacity(manifest.bytes);
        for index in 0..manifest.chunks {
            let chunk = self
                .get_object(&chunk_key(id, index))?
                .ok_or_else(|| format!("MAIL4024: committed message {id:?} is missing chunk {index}"))?;
            let observed = self.sha256(&chunk)?;
            if !observed.eq_ignore_ascii_case(&manifest.chunk_sha256[index]) {
                return Err(format!(
                    "MAIL4021: stored message {id:?} chunk {index} failed SHA-256 verification"
                ));
            }
            if raw.len().saturating_add(chunk.len()) > MAX_MESSAGE_BYTES {
                return Err("MAIL4021: stored message exceeds maximum reconstructed size".into());
            }
            raw.extend_from_slice(&chunk);
        }
        if raw.len() != manifest.bytes {
            return Err(format!(
                "MAIL4021: stored message {id:?} length mismatch: manifest={}, reconstructed={}",
                manifest.bytes,
                raw.len()
            ));
        }

        let metadata = self
            .get_object(&metadata_key(id))?
            .ok_or_else(|| format!("MAIL4024: committed message {id:?} is missing metadata"))?;
        if metadata.len() > MAX_METADATA_BYTES {
            return Err("MAIL4021: stored message metadata exceeds maximum size".into());
        }
        let metadata_json = String::from_utf8(metadata)
            .map_err(|_| "MAIL4021: stored message metadata is not UTF-8".to_string())?;

        Ok(Some(StoredMail {
            id: manifest.id,
            folder: manifest.folder,
            raw,
            metadata_json,
        }))
    }

    pub fn list(&self, folder: MailFolder, limit: usize) -> Result<Vec<String>, String> {
        if limit == 0 || limit > MAX_LIST_RESULTS {
            return Err(format!(
                "MAIL1002: mail-store list limit must be in 1..={MAX_LIST_RESULTS}"
            ));
        }

        // RBE storage prefixes are safe storage keys, not directory syntax; a
        // trailing slash would create an empty path component and be rejected.
        let prefix = folder.as_str();
        let payload = format!(
            "{{\"prefix\":{},\"limit\":{limit}}}",
            json_string(prefix)
        );
        let reply = self.call_storage("list", payload.as_bytes(), "MAIL4020")?;
        let text = reply_text(&reply, "storage list")?;
        let expected_prefix = format!("{prefix}/");
        let mut ids = Vec::new();
        for key in string_array_field(text, "keys")? {
            let Some(rest) = key.strip_prefix(&expected_prefix) else {
                return Err("MAIL4021: storage list escaped requested mail folder prefix".into());
            };
            let Some(id) = rest.strip_suffix(".ref") else {
                continue;
            };
            validate_id(id)?;
            ids.push(id.to_string());
        }
        ids.sort();
        ids.dedup();
        Ok(ids)
    }

    /// Repair only a folder index pointer for an already committed *and fully
    /// verified* message. A manifest by itself is not enough authority to make
    /// a message visible again.
    pub fn repair_index(&self, id: &str) -> Result<StoredMailSummary, String> {
        validate_id(id)?;
        let stored = self
            .get(id)?
            .ok_or_else(|| format!("MAIL4024: message {id:?} has no committed manifest"))?;
        let manifest_bytes = self
            .get_object(&manifest_key(id))?
            .ok_or_else(|| format!("MAIL4024: message {id:?} lost its commit manifest during index repair"))?;
        let manifest = decode_manifest(id, &manifest_bytes)?;
        if stored.folder != manifest.folder || stored.raw.len() != manifest.bytes {
            return Err("MAIL9001: mail-store verified state changed during index repair".into());
        }
        self.put_object(&index_key(manifest.folder, id), id.as_bytes())?;
        Ok(summary_from_manifest(&manifest))
    }

    /// Make the message invisible first, then remove committed state and chunks.
    /// Any failure after manifest deletion can leave only unreachable orphan data.
    pub fn delete(&self, id: &str) -> Result<bool, String> {
        validate_id(id)?;
        let Some(manifest_bytes) = self.get_object(&manifest_key(id))? else {
            return Ok(false);
        };
        let manifest = decode_manifest(id, &manifest_bytes)?;

        // The folder pointer is visibility. If removing it fails, abort before
        // deleting the commit record so list/get cannot disagree about a
        // dangling visible id.
        self.delete_object(&index_key(manifest.folder, id))?;
        self.delete_object(&manifest_key(id))?;

        let mut cleanup_error = None;
        if let Err(error) = self.delete_object(&metadata_key(id)) {
            cleanup_error = Some(error);
        }
        for index in 0..manifest.chunks {
            if let Err(error) = self.delete_object(&chunk_key(id, index)) {
                cleanup_error.get_or_insert(error);
            }
        }
        if let Some(error) = cleanup_error {
            return Err(format!(
                "MAIL4025: message {id:?} was uncommitted but orphan cleanup was incomplete: {error}"
            ));
        }
        Ok(true)
    }

    fn put_object(&self, key: &str, data: &[u8]) -> Result<(), String> {
        let payload = format!(
            "{{\"key\":{},\"data_hex\":{}}}",
            json_string(key),
            json_string(&hex_encode(data))
        );
        self.call_storage("put", payload.as_bytes(), "MAIL4020")?;
        Ok(())
    }

    fn get_object(&self, key: &str) -> Result<Option<Vec<u8>>, String> {
        let payload = format!("{{\"key\":{}}}", json_string(key));
        let reply = self.call_storage("get", payload.as_bytes(), "MAIL4020")?;
        let text = reply_text(&reply, "storage get")?;
        if !bool_field(text, "found")? {
            return Ok(None);
        }
        Ok(Some(hex_decode(&string_field(text, "data_hex")?)?))
    }

    fn exists(&self, key: &str) -> Result<bool, String> {
        let payload = format!("{{\"key\":{}}}", json_string(key));
        let reply = self.call_storage("exists", payload.as_bytes(), "MAIL4020")?;
        bool_field(reply_text(&reply, "storage exists")?, "exists")
    }

    fn delete_object(&self, key: &str) -> Result<bool, String> {
        let payload = format!("{{\"key\":{}}}", json_string(key));
        let reply = self.call_storage("delete", payload.as_bytes(), "MAIL4020")?;
        bool_field(reply_text(&reply, "storage delete")?, "deleted")
    }

    fn sha256(&self, data: &[u8]) -> Result<String, String> {
        let payload = format!("{{\"data_hex\":{}}}", json_string(&hex_encode(data)));
        let reply = self
            .bridge
            .call(HostCall::new(
                CRYPTO_CAPABILITY,
                CRYPTO_CAPABILITY,
                "sha256",
                payload.as_bytes(),
            ))
            .map_err(|error| format!("MAIL4021: RBE SHA-256 verification failed: {error}"))?;
        let digest = string_field(reply_text(&reply, "crypto sha256")?, "digest_hex")?;
        if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("MAIL8001: RBE crypto SHA-256 response is not a 64-character hex digest".into());
        }
        Ok(digest.to_ascii_lowercase())
    }

    fn call_storage(
        &self,
        operation: &str,
        payload: &[u8],
        code: &str,
    ) -> Result<HostReply, String> {
        self.bridge
            .call(HostCall::new(
                STORAGE_CAPABILITY,
                STORAGE_CAPABILITY,
                operation,
                payload,
            ))
            .map_err(|error| format!("{code}: RBE storage {operation} failed: {error}"))
    }
}

fn summary_from_manifest(manifest: &Manifest) -> StoredMailSummary {
    StoredMailSummary {
        id: manifest.id.clone(),
        folder: manifest.folder,
        bytes: manifest.bytes,
        chunks: manifest.chunks,
    }
}

fn decode_manifest(expected_id: &str, bytes: &[u8]) -> Result<Manifest, String> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| "MAIL4021: stored mail manifest is not UTF-8".to_string())?;
    let manifest = parse_manifest(text)?;
    if manifest.id != expected_id {
        return Err("MAIL4021: stored mail manifest id does not match requested id".into());
    }
    if manifest.chunks == 0
        || manifest.chunks != manifest.chunk_sha256.len()
        || manifest.bytes == 0
        || manifest.bytes > MAX_MESSAGE_BYTES
    {
        return Err("MAIL4021: stored mail manifest has impossible size/chunk metadata".into());
    }
    Ok(manifest)
}

fn manifest_key(id: &str) -> String {
    format!("messages/{id}/manifest.json")
}

fn metadata_key(id: &str) -> String {
    format!("messages/{id}/metadata.json")
}

fn chunk_key(id: &str, index: usize) -> String {
    format!("messages/{id}/chunks/{index:06}.bin")
}

fn index_key(folder: MailFolder, id: &str) -> String {
    format!("{}/{id}.ref", folder.as_str())
}

fn validate_id(id: &str) -> Result<(), String> {
    if id.is_empty()
        || id.len() > 96
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        Err("MAIL1002: mail storage id must be 1..=96 ASCII letters/digits/dot/underscore/hyphen".into())
    } else {
        Ok(())
    }
}

fn encode_manifest(manifest: &Manifest) -> String {
    let digests = manifest
        .chunk_sha256
        .iter()
        .map(|digest| json_string(digest))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"version\":1,\"id\":{},\"folder\":{},\"bytes\":{},\"chunks\":{},\"chunk_sha256\":[{}]}}",
        json_string(&manifest.id),
        json_string(manifest.folder.as_str()),
        manifest.bytes,
        manifest.chunks,
        digests
    )
}

fn parse_manifest(input: &str) -> Result<Manifest, String> {
    let version = usize_field(input, "version")?;
    if version != 1 {
        return Err(format!(
            "MAIL4021: unsupported stored mail manifest version {version}"
        ));
    }
    let id = string_field(input, "id")?;
    validate_id(&id)?;
    let folder = MailFolder::parse(&string_field(input, "folder")?)?;
    let bytes = usize_field(input, "bytes")?;
    let chunks = usize_field(input, "chunks")?;
    let chunk_sha256 = string_array_field(input, "chunk_sha256")?;
    for digest in &chunk_sha256 {
        if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("MAIL4021: stored mail manifest contains an invalid chunk SHA-256".into());
        }
    }
    Ok(Manifest {
        id,
        folder,
        bytes,
        chunks,
        chunk_sha256,
    })
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
        return Err(format!(
            "MAIL8001: RBE response field {field:?} is not a string"
        ));
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
    Err(format!(
        "MAIL8001: RBE response field {field:?} contains an unterminated string"
    ))
}

fn bool_field(input: &str, field: &str) -> Result<bool, String> {
    let tail = field_tail(input, field)?;
    if tail.starts_with("true") {
        Ok(true)
    } else if tail.starts_with("false") {
        Ok(false)
    } else {
        Err(format!(
            "MAIL8001: RBE response field {field:?} is not boolean"
        ))
    }
}

fn usize_field(input: &str, field: &str) -> Result<usize, String> {
    let tail = field_tail(input, field)?;
    let end = tail
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(tail.len());
    if end == 0 {
        return Err(format!(
            "MAIL8001: RBE response field {field:?} is not an unsigned integer"
        ));
    }
    tail[..end]
        .parse::<usize>()
        .map_err(|_| format!("MAIL8001: RBE response field {field:?} is out of range"))
}

fn string_array_field(input: &str, field: &str) -> Result<Vec<String>, String> {
    let tail = field_tail(input, field)?;
    let Some(open) = tail.find('[') else {
        return Err(format!(
            "MAIL8001: RBE response field {field:?} is not an array"
        ));
    };
    let mut chars = tail[open + 1..].chars().peekable();
    let mut values = Vec::new();
    loop {
        while matches!(chars.peek(), Some(ch) if ch.is_whitespace() || *ch == ',') {
            chars.next();
        }
        match chars.peek().copied() {
            Some(']') => {
                chars.next();
                break;
            }
            Some('"') => {
                chars.next();
                let mut value = String::new();
                let mut escaped = false;
                let mut closed = false;
                for ch in chars.by_ref() {
                    if escaped {
                        match ch {
                            '"' => value.push('"'),
                            '\\' => value.push('\\'),
                            '/' => value.push('/'),
                            'n' => value.push('\n'),
                            'r' => value.push('\r'),
                            't' => value.push('\t'),
                            other => value.push(other),
                        }
                        escaped = false;
                    } else if ch == '\\' {
                        escaped = true;
                    } else if ch == '"' {
                        closed = true;
                        break;
                    } else {
                        value.push(ch);
                    }
                }
                if !closed {
                    return Err(format!(
                        "MAIL8001: RBE response field {field:?} contains an unterminated string"
                    ));
                }
                values.push(value);
            }
            _ => {
                return Err(format!(
                    "MAIL8001: RBE response field {field:?} contains a non-string item"
                ));
            }
        }
    }
    Ok(values)
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

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn hex_decode(value: &str) -> Result<Vec<u8>, String> {
    if value.len() % 2 != 0 {
        return Err("MAIL8001: RBE storage data_hex has odd length".into());
    }
    let mut out = Vec::with_capacity(value.len() / 2);
    let bytes = value.as_bytes();
    for index in (0..bytes.len()).step_by(2) {
        let high = hex_value(bytes[index])?;
        let low = hex_value(bytes[index + 1])?;
        out.push((high << 4) | low);
    }
    Ok(out)
}

fn hex_value(byte: u8) -> Result<u8, String> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err("MAIL8001: RBE storage data_hex contains non-hexadecimal data".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_round_trip_preserves_chunk_identity() {
        let manifest = Manifest {
            id: "msg-1".into(),
            folder: MailFolder::Inbox,
            bytes: 400_000,
            chunks: 3,
            chunk_sha256: vec!["a".repeat(64), "b".repeat(64), "c".repeat(64)],
        };
        let encoded = encode_manifest(&manifest);
        assert_eq!(parse_manifest(&encoded).unwrap(), manifest);
    }

    #[test]
    fn ids_reject_storage_path_escape() {
        assert!(validate_id("safe-message_01").is_ok());
        assert!(validate_id("../secret").is_err());
        assert!(validate_id("a/b").is_err());
        assert!(validate_id("").is_err());
    }

    #[test]
    fn folder_prefix_is_a_valid_rbe_storage_key() {
        for folder in [
            MailFolder::Inbox,
            MailFolder::Sent,
            MailFolder::Queue,
            MailFolder::Failed,
        ] {
            assert!(!folder.as_str().ends_with('/'));
        }
    }
}
