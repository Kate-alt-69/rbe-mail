# Package Error Book

`catalog.json` follows RBE Error Code Book schema version 1. `mail.md` is the long-form reference and `diagnostics.json` provides machine-readable cause/fix/retry metadata for EPER/package tooling.

Errors emitted by this package/build helper use the stable `MAILxxxx` code as the first token so RBE/EPER and logs can classify failures without parsing prose.

Diagnostic output must never include provider API keys, SMTP passwords, AWS secret keys/session tokens, or DKIM private keys.
