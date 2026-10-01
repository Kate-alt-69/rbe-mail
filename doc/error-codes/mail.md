# mail Error Code Book

`MAILxxxx` is the stable diagnostic namespace for the RBE `mail` package. Codes are never recycled after release.

Every entry states what failed, how to fix it, and whether retry is safe. Never include API keys, SMTP passwords, AWS secrets/session tokens, or DKIM private keys in diagnostics.

| Code | What failed | Retry | Fix |
|---|---|---|---|
| `MAIL1001` | Invalid email address | `never` | Use a trimmed ASCII dot-atom address such as user@example.com.<br>Remove display/header control characters. Use Address::named() only with a normal display name. |
| `MAIL1002` | Invalid message | `never` | Read the concrete error message and correct that message field before sending.<br>Do not retry unchanged invalid input. |
| `MAIL1003` | Invalid mail domain | `never` | Use a fully-qualified domain such as example.com or mail.example.com.<br>Remove schemes, paths, underscores, spaces, and leading/trailing hyphens from DNS labels. |
| `MAIL1004` | Invalid transport configuration | `never` | Validate the selected provider transport, sender identity, region/domain and credentials.<br>Use provider API credentials for API transports and provider SMTP credentials for SMTP transports. |
| `MAIL1005` | Invalid provider API path | `never` | Pass an absolute provider-local path beginning with one /, for example /emails.<br>Do not pass full URLs to provider-native request_json(). |
| `MAIL1006` | Unsupported provider HTTP method | `never` | Use a method supported by both the provider endpoint and the RBE net:http broker. |
| `MAIL1007` | Provider credential missing | `never` | Supply the provider credential through authorized RBE configuration<br>never hard-code it in source.<br>For SMTP, use SMTP credentials when the provider distinguishes them from API keys. |
| `MAIL1008` | Invalid provider resource identifier | `never` | No recovery guidance until emitted. |
| `MAIL1009` | Invalid Mailgun domain | `never` | Pass the verified Mailgun sending domain only, not a URL/path. |
| `MAIL1010` | Invalid SendPulse domain | `never` | Pass only the domain registered/verified in SendPulse. |
| `MAIL1011` | Invalid AWS region | `never` | Use an AWS region identifier such as ap-south-1 and ensure the SES identity exists in that region. |
| `MAIL1012` | Invalid SendGrid legacy template id | `never` | Use a valid legacy template id only when the upstream SendGrid legacy-template feature is still appropriate. |
| `MAIL1013` | Invalid SMTP endpoint | `never` | Use the provider-documented SMTP hostname and port.<br>Prefer STARTTLS/TLS profiles and never downgrade to plaintext to bypass MAIL2004/MAIL4010. |
| `MAIL1014` | SMTP credential missing | `never` | Open the provider SMTP settings and use its documented SMTP username/password/token pair. |
| `MAIL1015` | Invalid DNS domain | `never` | Pass a fully-qualified public domain such as gmail.com. |
| `MAIL1016` | Header injection rejected | `never` | Keep untrusted/user text in the body. Validate/encode structured header values before passing them to mail. |
| `MAIL1017` | Invalid template variable | `never` | Use simple variable names such as first_name or order-id. |
| `MAIL1018` | Incomplete self-hosted setup | `never` | Use setup_records_with_token() with valid example.com / mail.example.com names, a public IPv4 address, and a secure verification token. |
| `MAIL1019` | Invalid ownership verification token | `never` | Generate 32 cryptographically random bytes outside the package and pass their 64-character hex encoding to setup_records_with_token().<br>Do not derive ownership tokens from timestamps, domains, counters or user ids. |
| `MAIL1020` | Invalid self-hosted public IPv4 address | `never` | Use the publicly routable IPv4 address that will actually receive SMTP for mail.<domain>.<br>If you need IPv6-only setup, use a future AAAA/ip6-aware setup surface rather than lying in an A/ip4 record. |
| `MAIL1021` | Invalid Message-ID or References chain | `never` | Use RFC-style Message-IDs such as <opaque-id@example.com>.<br>Pass References as whitespace-separated Message-IDs only. |
| `MAIL1022` | Invalid unsubscribe target | `never` | For RFC 8058 one-click, use an HTTPS unsubscribe URI.<br>For non-one-click List-Unsubscribe, https:// or mailto: is accepted by this helper. |
| `MAIL1023` | Invalid OTP content | `never` | Keep product/code at 128 characters or fewer and expiry between 1 and 1440 minutes.<br>HTML is escaped automatically<br>do not pre-inject markup into product/code. |
| `MAIL2001` | Required RBE capability missing | `never` | Inspect the package approval/grant state and approve the required capability when RBE supports it.<br>Reactivate/restart the package session after grants change. |
| `MAIL2002` | Self-hosted server prerequisite missing | `never` | Call self_hosted_capabilities() and enable every missing prerequisite.<br>Use a provider transport until current RBE supplies the missing package-host primitives. |
| `MAIL2003` | TCP capability missing | `never` | Do not bypass RBE with std::net/raw sockets. Use provider HTTP mode until RBE net:tcp is implemented and granted. |
| `MAIL2004` | TLS capability missing | `never` | Never downgrade to plaintext SMTP. Use provider HTTPS mode or wait for RBE TLS authority. |
| `MAIL2005` | DNS capability missing | `never` | Approve net:dns for mail and reactivate the package session. |
| `MAIL2006` | RBE Library Host session unavailable | `never` | Execute the component through RBE/RPX package activation, not as a standalone binary.<br>Check SDK ABI/session logs if the package should already be hosted. |
| `MAIL2007` | Secure verification-token generation unavailable | `never` | Generate 32 random bytes with a trusted CSPRNG outside the package and call setup_records_with_token() with 64 hex characters.<br>Do not fall back to timestamp-derived tokens. |
| `MAIL4001` | Provider authentication rejected | `never` | Verify/rotate the provider credential and sender/domain permissions.<br>Do not retry unchanged credentials. |
| `MAIL4002` | Provider rate limit exceeded | `backoff` | Honor Retry-After/provider backoff semantics and reduce request concurrency. |
| `MAIL4003` | Provider quota or billing limit reached | `never` | Fix provider billing/quota/account state before retrying. |
| `MAIL4004` | Provider rejected the mail request | `conditional` | Inspect provider-detail and correct the rejected field/policy before retrying. |
| `MAIL4005` | Provider temporarily unavailable | `backoff` | Retry with exponential backoff, or deliberately switch provider if your application supports that policy. |
| `MAIL4006` | Provider request timed out | `ambiguous` | Treat delivery as ambiguous: query provider/idempotency state before resending.<br>Use an idempotency key where the provider supports it. |
| `MAIL4007` | RBE mail network operation failed | `conditional` | Inspect the underlying RBE broker diagnostic (HTTP3000/DNS3000/etc.).<br>Retry only if the failed operation is known to be safe/idempotent. |
| `MAIL4008` | RBE request or response limit exceeded | `never` | Reduce/split message, attachment or provider request data so it fits the active RBE broker limits.<br>Do not blindly retry the same oversized payload. |
| `MAIL4009` | Recipient rejected | `conditional` | No recovery guidance until emitted. |
| `MAIL4010` | SMTP TLS/STARTTLS unavailable | `never` | Do not send plaintext. Use provider HTTPS mode until RBE TLS/STARTTLS is implemented/granted. |
| `MAIL4011` | Inbound SMTP listener unavailable | `never` | Enable the RBE package listener primitive when implemented<br>keep server mode disabled meanwhile. |
| `MAIL4012` | Package mail.service unavailable | `never` | Use provider-only mode until RBE can register/activate package-owned mail.service. |
| `MAIL4013` | DNS lookup failed | `backoff` | Check the recipient/domain DNS externally and inspect the underlying DNS3000 detail.<br>Retry transient DNS failures with backoff. |
| `MAIL4014` | No mail exchanger or address fallback | `never` | Correct the recipient domain/address. This is not fixed by retrying the same invalid domain. |
| `MAIL4015` | Recipient domain publishes null MX | `never` | Do not retry. Correct the recipient/domain or use another contact address. |
| `MAIL5001` | Build prerequisite missing | `never` | Install/restore the prerequisite named in the error and rerun build.ps1/build.sh. |
| `MAIL5002` | Latest green RBE commit could not be resolved | `conditional` | Check GitHub access or set GITHUB_TOKEN.<br>Pass -RbeSha/--rbe-sha with a known-green full SHA as an explicit fallback. |
| `MAIL5003` | RBE source checkout or backend build failed | `conditional` | Read the underlying Git/RBE compiler output.<br>Delete .cache/rbe/upstream to refresh a corrupt checkout and retry. |
| `MAIL5004` | RBE SDK bootstrap or status failed | `conditional` | Inspect backend install/sdk status output and repair/reinstall .rbe.<br>If CLI syntax changed upstream, compare against backend install help. |
| `MAIL5005` | RPX validation or package compilation failed | `conditional` | Fix the first compiler/RPX/RBE diagnostic printed above MAIL5005 and rerun.<br>Use host-toolchain escape hatch only for deliberate local development when no managed toolchain exists. |
| `MAIL8001` | Malformed external or host response | `never` | Preserve the sanitized raw detail, package version and RBE commit<br>report compatibility drift.<br>Do not guess status/success when required metadata is malformed. |
| `MAIL8002` | Malformed DNS response | `never` | Preserve the DNS response/RBE version and report the incompatibility.<br>Do not proceed to SMTP with malformed MX data. |
| `MAIL9001` | Mail package invariant violated | `never` | Report the code, MailError::diagnostic(), package version and RBE commit with a minimal reproducer. |
| `MAIL9002` | Invalid system clock | `never` | Correct the host clock/time synchronization and retry.<br>For SES, also ensure clock skew is within AWS signing tolerance. |

## Build/bootstrap codes

`MAIL5001`–`MAIL5005` come from `build.ps1` / `build.sh`; fix the first lower-level Git/RBE/RPX diagnostic printed immediately above the wrapper code.

## Provider timeout rule

`MAIL4006` is deliberately marked `ambiguous`: a timeout can happen after a provider accepted the request. Check provider/idempotency state before resending.

## Self-hosted rule

Self-hosted SMTP stays fail-closed until RBE actually grants the required TCP/TLS/listener/storage/crypto/package-service authority. SDK method names are not proof of host authority.
