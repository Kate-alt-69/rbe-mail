fn valid_message_id(value: &str) -> bool {
    let value = value.trim();
    if value.len() < 5 || value.len() > 998 || !value.starts_with('<') || !value.ends_with('>') {
        return false;
    }
    if value.chars().any(char::is_control) || value.bytes().any(|byte| byte.is_ascii_whitespace()) {
        return false;
    }
    let inner = &value[1..value.len() - 1];
    let Some((left, right)) = inner.split_once('@') else { return false; };
    !left.is_empty() && !right.is_empty() && !right.contains('@')
}

fn valid_references(value: &str) -> bool {
    let mut count = 0usize;
    for item in value.split_ascii_whitespace() {
        if !valid_message_id(item) { return false; }
        count += 1;
        if count > 100 { return false; }
    }
    count > 0
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadHeaders {
    pub in_reply_to: String,
    pub references: String,
}

pub fn reply(parent_message_id: &str, existing_references: Option<&str>) -> Result<ThreadHeaders, String> {
    if !valid_message_id(parent_message_id) {
        return Err("MAIL1021: parent Message-ID is invalid; expected a single <local@domain> identifier without whitespace/control characters".into());
    }
    let references = match existing_references {
        Some(value) if valid_references(value) => format!("{} {parent_message_id}", value.trim()),
        Some(_) => return Err("MAIL1021: References header is invalid; pass a whitespace-separated list of <local@domain> Message-IDs".into()),
        None => parent_message_id.to_string(),
    };
    Ok(ThreadHeaders { in_reply_to: parent_message_id.into(), references })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unstructured_parent_id() {
        assert!(reply("abc@example.com", None).is_err());
        assert!(reply("<abc@example.com>", None).is_ok());
    }
}
