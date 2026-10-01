# Self-hosted mail mode

`Transport::self_hosted(domain, from)` is the self-hosted transport entry point.

The design is deliberately capability-gated. `mail` must never bypass RBE authority with raw OS sockets, filesystem access, or unmanaged key storage.

## Outbound path

1. validate sender/domain;
2. resolve recipient MX through `net:dns`;
3. connect to a public MX through `net:tcp`;
4. negotiate SMTP and STARTTLS through RBE-approved TLS authority;
5. DKIM-sign through managed crypto;
6. queue/store delivery state through package storage;
7. retry only SMTP failures whose delivery semantics are safe to retry.

## Inbound path

When RBE exposes package listener authority, `mail.service` will bind the configured SMTP listener, validate recipients for verified domains, parse MIME, authenticate SPF/DKIM/DMARC, and persist raw `.eml` plus normalized mailbox metadata.

## Domain verification

Self-hosted mode uses a DNS ownership challenge such as:

```text
_rbe-mail.example.com TXT rbe-mail-verification=<random challenge>
```

That proves DNS control. The standalone `self-hosted` component deliberately does **not** derive this token from a timestamp. Until RBE exposes package crypto/random authority, `setup_records()` fails with `MAIL2007`; generate 32 cryptographically random bytes externally and call `setup_records_with_token(..., <64-hex-token>)`.

DKIM is a separate key pair; the private key must stay under RBE-managed crypto/storage once that package authority is available, and the public key is published under a selector such as `rbe1._domainkey.example.com`.

MX, SPF, DKIM, DMARC, forward DNS, reverse DNS/PTR, TLS, inbound port availability, and outbound port availability are separate readiness checks.

## Current RBE package-host status

At the time this source was last audited, Backend grants/dispatches package `log`, `net:http`, and `net:dns`. `net:tcp`, TLS/STARTTLS, inbound listen/accept, package storage/crypto, and package-owned Service activation are still treated as unavailable even though some names/helpers exist in the SDK. SDK convenience is not host authority.

## Fail-closed behavior

Missing prerequisites emit `MAIL2xxx`/`MAIL4xxx` errors documented in the package Error Book. Server mode must not claim readiness while TLS, listener authority, storage, crypto, or package-service activation are missing.
