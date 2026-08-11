# ADR-0012: sane-airscan is the behavior oracle

- Status: accepted
- Date: 2026-08-11

## Context

The eSCL specification is a click-through download from Mopria; the WSD scan
documentation on Microsoft Learn is partially decaying, with four of six reference
pages now redirecting to an index or gone
([M0 ground truth](../M0-ground-truth.md)). Neither document describes what devices
*do*: the field knowledge — which firmware omits which XML element, which device
needs a delay after job creation, which "supported" resolution jams the ADF — exists
in exactly one open place, sane-airscan, accumulated over years of user reports at
20,643 lines of C (commit 484ede7).

sane-airscan is GPL-2.0-or-later; this workspace is MIT OR Apache-2.0. Its *code*
cannot cross that boundary. Its *behavioral knowledge* — facts about devices, not
expression — can, with attribution, and pretending otherwise would mean re-suffering
a decade of user-reported breakage out of licensing squeamishness about facts.

## Decision

sane-airscan is this project's behavior oracle: where the specification is silent,
ambiguous, or contradicted by devices, what sane-airscan does is the presumed-correct
answer.

Three rules operationalize it:

1. **Citations are pinned.** A quirk or interoperability decision taken from
   sane-airscan cites file and commit (for example
   `airscan-escl.c@484ede7`) in the code comment or test. An unpinned "sane-airscan
   does this" is not a citation.
2. **Divergence is an ADR appendix.** Deciding to behave *differently* from the
   oracle — because the spec is explicit, or because the oracle's behavior is itself a
   bug — requires appending the case to this ADR, so the list of known divergences is
   one document rather than folklore.
3. **Never code.** Reading GPL source to learn a fact is allowed and encouraged;
   porting its expression is forbidden (CONTRIBUTING.md states the boundary).
   Implementations are written from the specifications and from this project's own
   recorded traffic.

The gate is mechanical, not cultural, to the extent a citation rule can be: the
divergence list below is CODEOWNERS-protected together with the rest of `docs/adr/`,
`just placeholder` keeps an uncited "temporary" workaround from hiding as a stub, and
review holds the citation format — recorded here as the one rule that remains
review-enforced.

## Consequences

scankit inherits the field knowledge of the most deployed driverless stack without
inheriting its license or its architecture, and every inherited decision is traceable
to its source, so a future maintainer can re-examine the upstream context when a quirk
stops making sense.

The oracle ages: sane-airscan development may slow, and its answers describe the C
stack's constraints, not this one's. The pinned-commit discipline is what makes aging
manageable — a citation names the state of knowledge at decision time, not a moving
target.

### Known divergences

None yet. Each future entry: the behavior, the oracle's answer with pin, this
project's answer, and why.
