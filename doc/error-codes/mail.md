# mail Error Code Book

`MAILxxxx` is the stable diagnostic namespace for the RBE `mail` package. Codes are never recycled after release.

Every emitted error should tell DX **what failed**, whether retry is safe, and the concrete recovery action. Never log API keys, SMTP passwords, AWS secrets/session tokens, DKIM private material, message bodies, or private recipient content.

| Code | What failed | Retry | Fix |
|---|---|---|---|
| `MAIL1001` | Invalid email address | never | Use a valid trimmed mailbox such as `user@example.com`; remove control/header characters. |
| `MAIL1002` | Invalid message | never | Correct the concrete message field named by the diagnostic. |
| `MAIL1003` | Invalid mail domain | never | Use a fully-qualified public DNS name with valid labels. |
| `MAIL1004` | Invalid transport configuration | never | Correct credentials, sender identity, region/domain or provider settings. |
| `MAIL1005` | Invalid provider API path | never | Use a provider-local absolute path beginning with one `/`, not a full URL. |
| `MAIL1006` | Unsupported provider HTTP method | never | Use a method supported by the provider and RBE `net:http`. |
| `MAIL1007` | Provider credential missing | never | Supply the correct credential through authorized configuration. |
| `MAIL1008` | Invalid provider resource identifier | never | Correct the provider resource identifier named by the diagnostic. |
| `MAIL1009` | Invalid Mailgun domain | never | Pass the verified Mailgun sending domain only. |
| `MAIL1010` | Invalid SendPulse domain | never | Pass the SendPulse-verified domain only. |
| `MAIL1011` | Invalid AWS region | never | Use a valid AWS region containing the SES identity. |
| `MAIL1012` | Invalid SendGrid legacy template id | never | Use a valid still-supported legacy template identifier. |
| `MAIL1013` | Invalid SMTP endpoint | never | Use the documented SMTP address/port and secure TLS policy. |
| `MAIL1014` | SMTP credential missing | never | Use provider SMTP credentials; they may differ from API credentials. |
| `MAIL1015` | Invalid DNS domain | never | Pass a fully-qualified public domain. |
| `MAIL1016` | Header injection rejected | never | Remove CR/LF/control characters from structured header values. |
| `MAIL1017` | Invalid template variable | never | Use a supported simple template variable name. |
| `MAIL1018` | Incomplete self-hosted setup | never | Supply valid mail DNS names, public IP and secure ownership token. |
| `MAIL1019` | Invalid ownership verification token | never | Generate 32 CSPRNG bytes and pass the 64-character hexadecimal encoding. |
| `MAIL1020` | Invalid self-hosted public IPv4 address | never | Use the publicly routable IPv4 address that actually receives SMTP. |
| `MAIL1021` | Invalid Message-ID or References chain | never | Use valid RFC-style Message-IDs and References values. |
| `MAIL1022` | Invalid unsubscribe target | never | RFC 8058 one-click requires HTTPS; only use supported schemes otherwise. |
| `MAIL1023` | Invalid OTP content | never | Keep OTP fields bounded and expiry in the documented range. |
| `MAIL2001` | Required RBE capability missing | never | Approve the requested capability and reactivate the package host session. |
| `MAIL2002` | Self-hosted server prerequisite missing | never | Call `self_hosted_capabilities()` and approve every missing prerequisite. |
| `MAIL2003` | TCP capability missing | never | Approve `net:tcp`; never bypass RBE with ambient/raw sockets. |
| `MAIL2004` | TLS capability missing | never | Approve both `net:tcp` and `net:tls`; never downgrade SMTP to plaintext. |
| `MAIL2005` | DNS capability missing | never | Approve `net:dns` and reactivate the package session. |
| `MAIL2006` | RBE Library Host session unavailable | never | Run through activated RBE/RPX package hosting and inspect ABI/session diagnostics. |
| `MAIL2007` | Secure verification-token generation unavailable | never | Approve `crypto` or provide a separately generated 256-bit token. |
| `MAIL4001` | Provider authentication rejected | never | Verify/rotate credentials and sender/domain permissions. |
| `MAIL4002` | Provider rate limit exceeded | backoff | Honor provider backoff/Retry-After and reduce concurrency. |
| `MAIL4003` | Provider quota or billing limit reached | never | Fix billing/quota/account state first. |
| `MAIL4004` | Provider rejected the mail request | conditional | Correct the rejected field or provider policy condition. |
| `MAIL4005` | Provider temporarily unavailable | backoff | Retry with exponential backoff or deliberately switch configured provider. |
| `MAIL4006` | Provider request timed out | ambiguous | Query provider/idempotency state before resending; acceptance may already have happened. |
| `MAIL4007` | RBE mail network operation failed | conditional | Inspect the underlying RBE broker diagnostic and retry only safe/idempotent operations. |
| `MAIL4008` | RBE request or response limit exceeded | never | Reduce or split the message/request to fit active broker limits. |
| `MAIL4009` | Recipient rejected | conditional | Correct the recipient or recipient-side policy before retrying. |
| `MAIL4010` | SMTP TLS/STARTTLS unavailable | never | Approve `net:tcp` + `net:tls`. If an MX does not advertise STARTTLS or the handshake fails, do **not** send plaintext. |
| `MAIL4011` | Inbound SMTP listener unavailable | never | Approve `net:tcp-listen` before starting inbound SMTP. |
| `MAIL4012` | Package `mail.service` unavailable | never | Approve `service:package` and ensure the verified package contains its `service/**/*.service` source. |
| `MAIL4013` | DNS lookup failed | backoff | Inspect recipient DNS and the underlying RBE DNS diagnostic; retry only transient resolver failures. |
| `MAIL4014` | No mail exchanger or address fallback | never | Correct the recipient domain; it has no usable MX or implicit A/AAAA fallback. |
| `MAIL4015` | Recipient domain publishes null MX | never | Do not retry: the domain explicitly declares that it accepts no email. |
| `MAIL4016` | Temporary SMTP failure | backoff | Retry only the affected recipient domain with exponential backoff; do not hammer the same MX immediately. |
| `MAIL4017` | Permanent SMTP rejection | never | Inspect the SMTP stage/status and correct sender, recipient, content, or remote policy. |
| `MAIL4018` | Partial self-hosted delivery | ambiguous | Some recipient domains already accepted the message. Retry **only** undelivered recipients/domains to avoid duplicates. |
| `MAIL4019` | SMTPUTF8 unsupported | never | Use ASCII envelope mailbox addresses until self-hosted delivery explicitly negotiates SMTPUTF8. |
| `MAIL5001` | Build prerequisite missing | never | Install or restore the named prerequisite and rerun the build helper. |
| `MAIL5002` | Latest green RBE commit could not be resolved | conditional | Check GitHub access/token or explicitly select a known-green full RBE SHA. |
| `MAIL5003` | RBE source checkout or backend build failed | conditional | Fix the first Git/RBE compiler diagnostic; refresh `.cache/rbe/upstream` if corrupt. |
| `MAIL5004` | RBE SDK bootstrap or status failed | conditional | Inspect `backend install` / SDK status and repair the project-local `.rbe` SDK. |
| `MAIL5005` | RPX validation or package compilation failed | conditional | Fix the first RPX/compiler diagnostic printed above the wrapper error. |
| `MAIL8001` | Malformed external or host response | never | Preserve sanitized response detail plus RBE/package versions and report compatibility drift. |
| `MAIL8002` | Malformed DNS response | never | Stop before SMTP and preserve DNS/RBE details; do not guess MX state. |
| `MAIL8003` | Malformed SMTP or TLS broker response | never | Stop the SMTP transaction and preserve sanitized protocol/RBE details; never infer delivery success. |
| `MAIL9001` | Mail package invariant violated | never | Report the code, package version, RBE commit, and a minimal reproducer. |
| `MAIL9002` | Invalid system clock | never | Correct host time synchronization before constructing mail Date headers or provider signatures. |

## Self-hosted direct SMTP

`Transport::SelfHosted` uses RBE-owned `net:dns`, `net:tcp`, and `net:tls` authority. Delivery resolves MX records, performs SMTP `EHLO`, requires `STARTTLS`, upgrades the same opaque RBE transport with host-owned certificate validation, sends a second `EHLO`, then performs `MAIL FROM`, `RCPT TO`, `DATA`, dot-stuffing, and a best-effort `QUIT`.

A failed STARTTLS handshake destroys the RBE transport; mail never falls back to plaintext. `MAIL4018` is deliberately ambiguous because other recipient domains may already have accepted the message.

## Build/bootstrap codes

`MAIL5001`–`MAIL5005` come from `build.ps1` / `build.sh`. Fix the first lower-level Git/RBE/RPX diagnostic printed immediately above the wrapper code.
