# ADR-0007: bytes, not images

- Status: accepted
- Date: 2026-08-11

## Context

A scanner returns pages in a format it chose from its own capability set: JPEG, PDF,
or occasionally raw. The tempting scope expansion is to "help": decode the JPEG,
normalize rotation, assemble a multi-page PDF, deskew. Every step of that help drags
in an image codec dependency, an opinion about color management, and a compatibility
surface this project does not want to own — and it destroys information, because the
device's original bytes are the only artifact a bug report can be verified against.

The M0 measurements sharpen the point: the candidate stack is 26 crates with TLS
included; one image crate family would multiply that, in a library whose security
posture (SECURITY.md) is "we parse untrusted network input with a minimal surface".

## Decision

A scanned page is delivered as `ScannedPage`: a `PageInfo` (the declared format,
dimensions and resolution as stated by the device, and the input source it came from)
plus the device's bytes, unmodified. scankit never decodes, re-encodes, rotates, or
assembles image data. What the device sent is what the caller gets, in the order the
device produced it.

One seat is reserved and nothing more: a future `scankit-image` crate, above the
facade, may someday offer decoding conveniences to callers who want them. It has no
row in the ALLOWED matrix today, so creating it is a visible architecture change.

The gate is mechanical, not cultural. `just purity` and `just deps` keep image codec
crates out of the entire shipped graph — no row permits one — and `just placeholder`
prevents a "temporary" decode helper from living as an unwritten stub.

## Consequences

Callers that want a PDF of a multi-page ADF run must build it themselves or wait for
`scankit-image`. The `skan` CLI writes one file per page in the device's format, which
is the honest primitive.

Bug reports and cassettes (ADR 0011) stay byte-exact: a quirk fix can assert on the
precise bytes a device produced, because nothing between the socket and the caller
rewrites them.

The declared format is the device's claim, not a verified fact. scankit passes the
claim through; a device that labels JPEG as PDF is an interoperability report and a
quirk entry (ADR 0012), not something the transport silently sniffs and corrects —
sniffing is exactly the kind of helpful lie this ADR exists to forbid.

The dependency gates that hold this were turned on at bootstrap, before any page ever
flowed through the stack.
