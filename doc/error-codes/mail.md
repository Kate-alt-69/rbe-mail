# mail Error Code Book

`MAILxxxx` is the stable diagnostic namespace for the RBE `mail` package. Codes follow RBE numeric conventions and are never recycled after release.

## MAIL1001

**Status:** Emitted.

**Invalid email address.** Input address is malformed or contains unsafe control characters.

**Action:** Check package input and configuration.

## MAIL1002

**Status:** Emitted.

**Invalid message.** Message subject/body/recipient requirements were not satisfied.

**Action:** Check package input and configuration.

## MAIL1003

**Status:** Emitted.

**Invalid mail domain.** Domain syntax is malformed for provider or self-hosted use.

**Action:** Check package input and configuration.

## MAIL1004

**Status:** Emitted.

**Invalid transport configuration.** A transport credential or package configuration is invalid.

**Action:** Check package input and configuration.

## MAIL2001

**Status:** Emitted.

**Required RBE capability missing.** The package requested an RBE host capability that was not granted.

**Action:** Check RBE capability grants and package/server configuration.

## MAIL2002

**Status:** Emitted.

**Self-hosted server prerequisite missing.** Self-hosted mode is selected but the complete secure SMTP path is not available.

**Action:** Check RBE capability grants and package/server configuration.

## MAIL4001

**Status:** Emitted.

**Provider authentication rejected.** The provider returned HTTP 401 or 403.

**Action:** Inspect provider/network status and retry only when the returned error says retryable.

## MAIL4002

**Status:** Emitted.

**Provider rate limit exceeded.** The provider returned HTTP 429; retry only according to provider semantics.

**Action:** Inspect provider/network status and retry only when the returned error says retryable.

## MAIL4003

**Status:** Emitted.

**Provider quota or billing limit reached.** The provider returned HTTP 402 or an equivalent quota failure.

**Action:** Inspect provider/network status and retry only when the returned error says retryable.

## MAIL4004

**Status:** Emitted.

**Provider rejected the mail request.** The provider rejected the request with a non-retryable response.

**Action:** Inspect provider/network status and retry only when the returned error says retryable.

## MAIL4005

**Status:** Emitted.

**Provider temporarily unavailable.** The provider returned a 5xx response and may be retryable.

**Action:** Inspect provider/network status and retry only when the returned error says retryable.

## MAIL4006

**Status:** Emitted.

**Provider request timed out.** The provider timed out. Delivery may be ambiguous; do not blindly resend.

**Action:** Inspect provider/network status and retry only when the returned error says retryable.

## MAIL4007

**Status:** Emitted.

**RBE mail network operation failed.** An RBE HTTP/DNS/TCP operation failed before mail completed.

**Action:** Inspect provider/network status and retry only when the returned error says retryable.

## MAIL4008

**Status:** Reserved.

**RBE request or response limit exceeded.** The message exceeds an RBE capability envelope/body limit.

**Action:** Inspect provider/network status and retry only when the returned error says retryable.

## MAIL4009

**Status:** Reserved.

**Recipient rejected.** SMTP/provider explicitly rejected a recipient.

**Action:** Inspect provider/network status and retry only when the returned error says retryable.

## MAIL4010

**Status:** Emitted.

**SMTP TLS/STARTTLS unavailable.** Direct SMTP is blocked because secure TLS upgrade authority is unavailable.

**Action:** Check RBE capability grants and package/server configuration.

## MAIL4011

**Status:** Emitted.

**Inbound SMTP listener unavailable.** Mail server mode cannot accept SMTP because package listen/accept authority is unavailable.

**Action:** Check RBE capability grants and package/server configuration.

## MAIL4012

**Status:** Emitted.

**Package mail.service unavailable.** Mail server mode cannot activate its package-owned service.

**Action:** Check RBE capability grants and package/server configuration.

## MAIL8001

**Status:** Emitted.

**Malformed external or host response.** Provider/RBE capability returned response metadata that mail cannot safely interpret.

**Action:** Preserve the diagnostic context and report it with the package version.

## MAIL9001

**Status:** Emitted.

**Mail package invariant violated.** An internal state reached an impossible dispatcher path; likely a mail package bug.

**Action:** Preserve the diagnostic context and report it with the package version.

## Provider-native and compatibility errors

### MAIL1005 — Invalid provider API path

Provider-native API path is not provider-local or attempts traversal.

### MAIL1006 — Unsupported provider HTTP method

Provider API escape hatch received an HTTP method outside the allowlist.

### MAIL1007 — Provider credential missing

A provider API or SMTP surface received an empty credential.

### MAIL1009 — Invalid Mailgun domain

Mailgun sending domain is invalid.

### MAIL1010 — Invalid SendPulse domain

SendPulse sender domain is invalid.

### MAIL1011 — Invalid AWS region

AWS SES region is empty or malformed.

### MAIL1012 — Invalid SendGrid legacy template id

SendGrid legacy transactional-template identifier is empty.

### MAIL1013 — Invalid SMTP endpoint

SMTP endpoint is incomplete or has an invalid port.

### MAIL1014 — SMTP credential missing

SMTP username/password is missing.

### MAIL1015 — Invalid DNS domain

DNS component received an invalid domain.

### MAIL1016 — Header injection rejected

MIME builder rejected CR/LF header injection.

### MAIL1017 — Invalid template variable

Template variable name contains unsupported characters.

### MAIL1018 — Incomplete self-hosted setup

Self-hosted setup requires domain, mail host and public IP.

### MAIL2003 — TCP capability missing

RBE net:tcp is not granted.

### MAIL2004 — TLS capability missing

RBE TLS/STARTTLS authority is not granted.

### MAIL2005 — DNS capability missing

RBE net:dns is not granted.

### MAIL4013 — DNS lookup failed

RBE failed to complete the requested DNS lookup.

### MAIL4014 — No MX records

Resolver returned no MX records.

### MAIL8002 — Malformed DNS response

RBE DNS broker response could not be decoded.

### MAIL9002 — Invalid system clock

Provider-signature time cannot be derived from the system clock.

