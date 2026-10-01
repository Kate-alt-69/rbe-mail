#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsubscribeHeaders {
    pub list_unsubscribe: String,
    pub list_unsubscribe_post: Option<&'static str>,
}

pub fn unsubscribe(url: &str, one_click: bool) -> Result<UnsubscribeHeaders, String> {
    let url = url.trim();
    if url.is_empty() || url.contains('\r') || url.contains('\n') {
        return Err("MAIL1022: invalid List-Unsubscribe target; remove whitespace/control characters".into());
    }
    if one_click {
        if !url.starts_with("https://") {
            return Err("MAIL1022: RFC 8058 one-click unsubscribe requires an HTTPS URI; mailto is not valid as the one-click target".into());
        }
    } else if !(url.starts_with("https://") || url.starts_with("mailto:")) {
        return Err("MAIL1022: List-Unsubscribe must use https:// or mailto:".into());
    }

    Ok(UnsubscribeHeaders {
        list_unsubscribe: format!("<{url}>"),
        list_unsubscribe_post: if one_click { Some("List-Unsubscribe=One-Click") } else { None },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_click_requires_https() {
        assert!(unsubscribe("mailto:unsubscribe@example.com", true).is_err());
        assert!(unsubscribe("https://example.com/unsubscribe/abc", true).is_ok());
    }
}
