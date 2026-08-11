# ADR-0010: no silent negotiation

- Status: accepted
- Date: 2026-08-11

## Context

A caller asks for 600 DPI duplex from the ADF; the device offers 300 DPI simplex.
Every scanner stack faces this moment, and the incumbent answer is silent nearest-fit:
send *something* the device accepts and let the user discover the substitution in the
output. SANE frontends do it, drivers do it, and the result is scans that are quietly
worse than requested — the failure mode users distrust most, because nothing reported
it.

Silent negotiation also destroys reproducibility, which this workspace's verification
strategy depends on: a cassette (ADR 0011) recorded through a stack that negotiates
invisibly documents neither what was asked nor why the wire shows something else.

## Decision

A `ScanTicket` that the device's capabilities cannot satisfy is a structured error
before any request is sent: the vocabulary (ADR 0006) names the mismatched dimension
and carries what was asked and what the source offers. Validation happens against the
per-source capability tables (ADR 0013) in the backend, so an unsatisfiable ticket
never reaches the wire.

Rounding exists in exactly one place: an explicit `nearest()` call on the ticket,
which the caller invokes deliberately, and which returns the adjusted ticket so the
caller holds — and can display — what will actually be requested. `nearest()` is
policy the caller opted into; it is never applied on the way to the device.

The gate is mechanical, not cultural. The M1 virtual-device suite (`just test`)
carries the mismatch table — each capability dimension with a ticket that misses it —
and asserts the structured error and the untouched wire; `just placeholder` keeps
`nearest()` and the validation path from shipping as stubs.

## Consequences

Callers that just want "scan with whatever works" write `ticket.nearest(&caps)?` and
get exactly the incumbent convenience, minus the silence — one line, and the adjusted
ticket in hand to show in a UI. The one-call facade API does exactly this, visibly, in
its documentation.

Validation requires the capability model to be honest about what devices actually
enforce, which is where reality bites: devices lie in both directions (advertising
what they refuse, accepting what they never advertised). The rule holds anyway:
scankit validates against the device's *stated* capabilities, and a device whose
statements are wrong gets a quirk entry with a pinned citation (ADR 0012) rather than
a loosened validator.

The mismatch table was specified at bootstrap as part of M1's exit criteria, before
the validator it tests existed.
