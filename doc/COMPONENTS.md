# mail package components

The package name is **mail**. The repository may be named `rbe-mail`.

RBE exposes every directory under `components/` as an independent package import.

```rel
:import[mail]
:import[resend-api from mail]
:import[sendgrid-api from mail]
```

## Unified

- `mail` — normalized send API across Resend, SendGrid, Brevo, Postmark, Mailgun, SendPulse and AWS SES, plus self-hosted readiness/outbound SMTP.

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

Provider SMTP components supply provider-correct endpoint/credential profiles. Secure transmission stays fail-closed: self-hosted outbound delivery requires RBE `net:tcp` + `net:tls` and never downgrades after STARTTLS failure.

## Self-hosted server primitives

- `smtp-server` — inbound RBE-owned listener/connection transport through `net:tcp-listen`, including bounded accept/read/write, inbound STARTTLS, ownership-token generation and Ed25519 helpers. It never opens ambient/raw host sockets.
- `mail-store` — durable package-scoped message persistence through RBE `storage` + `crypto`. Raw RFC5322 messages are chunked, SHA-256 pinned, committed by a manifest written last, then exposed through inbox/sent/queue/failed index pointers. `repair_index()` only re-exposes a message after complete chunk verification.
- `self-hosted` — DNS setup/readiness helpers and capability checks for full server mode.

`mail-store` treats `messages/<id>/manifest.json` as the commit record. Folder `.ref` entries are indexes only; a partial crash before the manifest is never considered a valid message.

## Utilities

- `provider-info`
- `dns`
- `mime`
- `webhook`
- `templates`
- `errors`
- `otp`
- `transactional`
- `marketing`
- `bulk`
- `threading`

Each component is deliberately independently compilable because RPX targeted Rust builds stage only the selected component directory.
