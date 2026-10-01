# mail

`mail` is an RBE-native email package written for the Rust RBE SDK.

The public goal is one small API regardless of whether delivery uses a third-party provider or RBE's self-hosted SMTP mode.

## Providers in v0.1

- Resend
- SendGrid
- Brevo
- Postmark
- Mailgun
- SendPulse
- AWS SES v2 (+ explicit legacy SES component)
- self-hosted RBE SMTP groundwork

All provider traffic uses RBE HostBridge capabilities. The package does not read process environment directly, open raw sockets behind RBE, or print directly to stdout.

## Simple API

Conceptually in REL:

```rel
:import[mail]

mail.use("resend", {
    key: ENV.require("RESEND_KEY"),
    from: "Kastrick <official@example.com>"
})

mail.send("kate@example.com", {
    subject: "HELLO :D",
    text: "yo what's up"
})
```

Rust SDK shape:

```rust
let mail = mail::create_library(
    host,
    mail::Transport::resend(
        resend_key,
        mail::Address::named("Kastrick", "official@example.com"),
    ),
)?;

mail.send(
    "kate@example.com",
    mail::Message::new("HELLO :D").text("yo what's up"),
)?;
```

Changing providers only changes `Transport`.

## Error Book / EPER

Every package error has a stable `MAILxxxx` diagnostic code. The package-local Error Book uses the same schema and numbering conventions as RBE's core Error Code Book:

- `MAIL1xxx` — invalid mail input/configuration
- `MAIL2xxx` — RBE capability/authority/configuration problems
- `MAIL4xxx` — live provider/SMTP/runtime failures
- `MAIL5xxx` — build/SDK/RPX bootstrap failures
- `MAIL8xxx` — malformed or incompatible external responses
- `MAIL9xxx` — package invariant failures / likely `mail` bugs

Machine catalog: `doc/error-codes/catalog.json`
Actionable EPER metadata: `doc/error-codes/diagnostics.json`
Long-form reference: `doc/error-codes/mail.md`

Current RBE `main` embeds its core error catalog at build time. The package catalog is intentionally packaged in the same machine-readable shape so EPER can ingest it once package-local Error Book discovery is wired into RBE. Until then the emitted `MAILxxxx` code and this local book remain authoritative.

## Self-hosted SMTP

Current RBE package-host authority provides `log`, `net:http`, and `net:dns`. The Rust SDK also names additional network/storage/crypto primitives, but those names are not authority: current Backend does not yet grant/dispatch the TCP/TLS/listener/storage/crypto/package-service pieces needed by a full SMTP server. `mail` therefore fails closed instead of bypassing RBE or silently falling back to plaintext SMTP.

Expected authority surface:

- `net:dns`
- `net:tcp`
- `net:tls`
- `net:tcp-listen`
- `storage`
- `crypto`
- `service:package`

See `doc/SELF_HOSTED.md`.

## Build

`mail` does **not** depend on a local RBE checkout.

Both build helpers resolve the latest successful `CI` run on RBE `main`, clone/update
`Kate-alt-69/RBE` into `.cache/rbe/upstream`, check out that exact green commit, and
compile only `backend`. The freshly-built backend then installs/updates the verified
project-local Rust SDK before RPX validates and packages `mail`.

Windows:

```powershell
.\build.ps1
```

Linux/WSL:

```bash
./build.sh
```

The SDK install call intentionally uses separate option/value tokens:

```text
backend install sdk.latest -path <repo> -language rust
```

Useful overrides:

```powershell
.\build.ps1 -RbeSha <40-char-sha>
.\build.ps1 -NoRbeRefresh
.\build.ps1 -NoSdkUpdate
.\build.ps1 -CheckOnly
.\build.ps1 -AllowHostToolchain
```

```bash
./build.sh --rbe-sha <40-char-sha>
./build.sh --no-rbe-refresh
./build.sh --no-sdk-update
./build.sh --check-only
./build.sh --allow-host-toolchain
```

`GITHUB_TOKEN` is optional and only raises the GitHub API rate limit. No token is
required for the public RBE repository.
