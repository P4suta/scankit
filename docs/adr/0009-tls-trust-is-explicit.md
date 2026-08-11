# ADR-0009: TLS trust is explicit

- Status: accepted
- Date: 2026-08-11

## Context

Scanners advertise `_uscans._tcp` and serve eSCL over HTTPS with certificates no
public authority signed: self-signed at the factory, regenerated on reset, bearing
names like `BRN30055C` that match no DNS record. A stack that validates against system
roots simply cannot connect to most secure scanners; every existing client therefore
"solves" this by accepting any certificate, silently, which reduces the S in HTTPS to
decoration and trains an entire ecosystem to ignore the one defense the LAN has
against impersonation — and a LAN with a hostile device on it is exactly the threat
model SECURITY.md takes seriously.

The temptation to hide this mess from callers is strong, and hiding it is precisely
the accept-all behavior being avoided. The caller must hold the decision, because only
the caller knows whether this is a home network, an office fleet with pinned
inventory, or a pentest.

## Decision

`scankit-http` owns a `TrustPolicy` type with exactly three variants:

- `SystemRoots` — validate against the platform trust store. Correct for the rare
  scanner with a real chain, and the right default *posture* for anything that is not
  a scanner.
- `TrustOnFirstUse` — accept and pin whatever the device presents on first contact
  (the caller supplies the pin store), and refuse any later change loudly. The SSH
  model, and the right posture for "my scanner at home".
- `Fingerprint` — trust exactly the caller-supplied certificate fingerprint. The right
  posture for managed fleets and for `scandev` cassette work.

There is no accept-all variant, no `danger_accept_invalid_certs()` escape hatch, and
no `Default` implementation: connecting over TLS requires writing a policy at the call
site. A TrustOnFirstUse pin mismatch is a dedicated error in the vocabulary
(ADR 0006), because "the device's identity changed" is a sentence a UI must be able to
say.

The gate is mechanical, not cultural: the type system carries it. A missing variant
cannot be called, and the absence of `Default` makes the compiler demand the decision.
`just deps` keeps alternative TLS stacks (with their own permissive knobs) out of the
graph, and `just placeholder` keeps the pin-mismatch path from shipping as a stub.

## Consequences

Every caller writes one more line than they would against an accept-all client, and
the facade's one-call API must thread a policy parameter through — a DevEx cost M5
must design well rather than remove.

Plain-HTTP eSCL and WSD remain available and untouched by this ADR; most devices offer
both, and refusing to fake security is not the same as demanding it.

TrustOnFirstUse requires a pin store, and scankit-core defines its trait rather than
its storage — the caller decides where pins live. The virtual test bed exercises the
mismatch path with a scripted certificate change, so the loud failure is tested, not
aspirational.

The type was frozen here at bootstrap, before the first TLS connection was ever made
by this stack.
