#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtpMessage {
    pub subject: String,
    pub text: String,
    pub html: String,
    pub code: String,
    pub expires_in_minutes: u16,
}

fn html_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

pub fn build(product: &str, code: &str, expires_in_minutes: u16) -> Result<OtpMessage, String> {
    let product = product.trim();
    let code = code.trim();
    if product.is_empty() || code.is_empty() {
        return Err("MAIL1023: OTP product/code cannot be empty".into());
    }
    if product.len() > 128 || code.len() > 128 || product.chars().any(char::is_control) || code.chars().any(char::is_control) {
        return Err("MAIL1023: OTP product/code is too long or contains control characters; keep both to 128 characters or fewer".into());
    }
    if expires_in_minutes == 0 || expires_in_minutes > 1440 {
        return Err("MAIL1023: OTP expiry must be between 1 and 1440 minutes".into());
    }

    let subject = format!("{product} verification code");
    let text = format!("Your {product} verification code is {code}. It expires in {expires_in_minutes} minutes.");
    let product_html = html_escape(product);
    let code_html = html_escape(code);
    let html = format!(
        "<p>Your <strong>{product_html}</strong> verification code is:</p><p><strong>{code_html}</strong></p><p>It expires in {expires_in_minutes} minutes.</p>"
    );
    Ok(OtpMessage { subject, text, html, code: code.into(), expires_in_minutes })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_html_in_product_and_code() {
        let message = build("<Kastrick>", "<123&456>", 10).unwrap();
        assert!(message.html.contains("&lt;Kastrick&gt;"));
        assert!(message.html.contains("&lt;123&amp;456&gt;"));
        assert!(!message.html.contains("<123&456>"));
    }
}
