# Durable mail storage Error Book

`MAIL4020`–`MAIL4025` belong to the `mail-store` component. The store uses RBE's package-scoped `storage` capability and RBE `crypto.sha256`; it never bypasses those authorities with ambient filesystem access.

The commit protocol is intentionally crash-safe: message chunks and metadata are written first, `messages/<id>/manifest.json` is written last as the commit record, and the folder pointer (`inbox/<id>.ref`, `queue/<id>.ref`, etc.) is written only after the commit record exists. A crash can therefore leave orphan data, but it cannot make partial bytes look like a committed message.

| Code | What failed | Safe recovery |
|---|---|---|
| `MAIL4020` | RBE package storage is unavailable or an operation failed. | Approve `storage`, reactivate the exact package session, and inspect the underlying RBE storage diagnostic. Retry only after authority/I/O is healthy. |
| `MAIL4021` | A committed manifest/chunk failed integrity validation, SHA-256 verification, size validation, or structural validation. | Stop processing that message id. Restore from an authoritative copy and verify every chunk; never guess or silently accept damaged bytes. |
| `MAIL4022` | The immutable message commit succeeded, but its folder index pointer failed. | **Do not write/send the message again.** Call `repair_index(id)`; it reconstructs the pointer from the committed manifest. |
| `MAIL4023` | The requested immutable storage id is already committed. | For an idempotent retry, load the existing record. Otherwise create a new id. Never overwrite an existing commit. |
| `MAIL4024` | A committed message is missing a required chunk, metadata object, or commit record. | Quarantine the id and recover from the original SMTP/provider source or another authoritative copy. |
| `MAIL4025` | Logical deletion succeeded, but some unreachable chunk/metadata cleanup failed. | Leave the message invisible and run bounded orphan cleanup later. Do not recreate its manifest/index merely to delete leftovers. |

## Diagnostic privacy

EPER/operator diagnostics may include the stable error code, immutable mail-store id, folder name, chunk index, expected/observed SHA-256, RBE version, package version, and the lower-level RBE storage/crypto error. They must not include raw email bodies, attachment bytes, API/SMTP credentials, DKIM private material, or private recipient content.

## Integrity model

A visible message is valid only when all of these hold:

1. `manifest.json` exists and parses as the supported manifest version.
2. The manifest id matches the requested immutable id.
3. `chunks` equals the number of pinned chunk digests.
4. Every chunk exists and re-hashes to the manifest's SHA-256.
5. Reconstructed byte length exactly matches the committed length.
6. Metadata exists, stays within its bound, and is UTF-8.

A folder pointer is an index only. It is never the authority that a message committed successfully; the commit manifest is.
