# Legacy compatibility policy

`mail` supports legacy behavior only while the upstream provider still documents and accepts it.

- **SendGrid:** legacy transactional templates remain supported through v3 and SMTP `X-SMTPAPI`. Use `sendgrid-api` and `sendgrid-legacy`.
- **Amazon SES:** the original SES API remains documented alongside SES API v2. Use `ses-legacy-api` for the `2010-12-01` Query API and `ses-api` for v2.
- **SendPulse:** the client-credentials OAuth flow remains available; `sendpulse-api` includes token acquisition while normal requests accept a bearer token.
- **SMTP:** provider SMTP profiles are included where SMTP relay is still offered.

The package will not:
- resurrect removed provider endpoints;
- silently downgrade from a modern API after an error;
- send credentials/content over plaintext merely to preserve old behavior.
