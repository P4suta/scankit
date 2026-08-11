## What this changes

<!-- One or two sentences. What behavior is different after this lands? -->

## Semantics

- [ ] No protocol grammar leaked out of `scankit-escl` / `scankit-wsd`, and no socket,
      clock, or sleep leaked into them; time is owned by the backends alone
- [ ] No ticket/capability mismatch is rounded silently; the only rounding path is an
      explicit `nearest()` call, and everything else is a structured error
- [ ] Pages remain the device's bytes with their declared format; nothing decodes,
      re-encodes, or "fixes" image data

<!--
docs/adr/0003, 0007, 0010. A quirk workaround belongs next to a pinned sane-airscan
citation (docs/adr/0012), not inline as an unexplained special case.
-->

## Safety

- [ ] No allocation is sized and no loop bounded by a length read out of a device's XML,
      SOAP, or datagram without a stated bound
- [ ] Arithmetic on device-derived counts states its overflow behavior; no unchecked
      indexing or slicing
- [ ] TLS trust changed nowhere, or the change is an explicit `TrustPolicy` variant the
      caller opts into (docs/adr/0009)

## Checks

- [ ] `just ci` passes locally
- [ ] The dependency graph gained no edge outside the ALLOWED matrix, and the sans-I/O
      crates gained no third party (`just deps`, `just purity`)
- [ ] No `allow` or `ignore` was added to make a gate pass (`just placeholder` would
      catch it; do not fight the gate)

<!--
If a gate was changed rather than satisfied, say why here. That is sometimes right, and it
always deserves a sentence. The legitimate route is the shared configuration — clippy.toml,
the workspace lints table, deny.toml — never a local suppression. See CONTRIBUTING.md.
-->
