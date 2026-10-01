# mail package components

The package name is **mail**. The repository may be named `rbe-mail`.

RBE exposes every directory under `components/` as an independent package import.

```rel
:import[mail]
:import[resend-api from mail]
:import[sendgrid-api from mail]
```

## Unified

- `mail` — normalized send API across Resend, SendGrid, Brevo, Postmark, Mailgun, SendPulse and AWS SES, plus self-hosted readiness.

## Provider-native APIs

- `resend-api`
- `sendgrid-api`
- `brevo-api`
- `postmark-api`
- `mailgun-api`
- `sendpulse-api`
- `ses-api`

These expose provider-specific operations plus a provider-local `request_json()` escape hatch. The provider hostname and authentication scheme are fixed by the component; callers cannot turn one into a general HTTP client.

## Compatibility / legacy

- `sendgrid-legacy` — legacy transactional-template and `X-SMTPAPI` helpers that SendGrid still documents.
- `ses-legacy-api` — original Amazon SES Query API (`2010-12-01`) compatibility.

Legacy means **still accepted/documented by the provider**, not “an endpoint that existed once.”

## SMTP compatibility

- `smtp`
- `resend-smtp`
- `sendgrid-smtp`
- `brevo-smtp`
- `postmark-smtp`
- `mailgun-smtp`
- `sendpulse-smtp`
- `ses-smtp`

Provider SMTP components supply provider-correct endpoint/credential profiles. Secure transmission remains fail-closed until RBE grants package TLS/STARTTLS authority.

## Utilities

- `provider-info`
- `dns`
- `mime`
- `webhook`
- `templates`
- `errors`
- `self-hosted`
- `otp`
- `transactional`
- `marketing`
- `bulk`
- `threading`

Each component is deliberately independently compilable because RPX targeted Rust builds stage only the selected component directory.
