# ADR-0005: one job borrows the scanner

- Status: accepted
- Date: 2026-08-11

## Context

A physical scanner is a mutex with paper in it. eSCL makes this explicit — a second
`CreateScanJob` while one is running is refused by the device — and WSD behaves the
same in practice. An API that lets a caller hold two live jobs against one device is
promising something the hardware will break at runtime, in the device's own
vocabulary, at the worst moment.

Concurrency bugs of this class are usually documented ("do not call scan while a job
is active") and then committed anyway. Rust can do better, because exclusive access is
what `&mut` means.

The second question is what dropping a job means. A job holds device-side state (an
eSCL job URI, a WSD job token); dropping the value cannot reliably release it, because
`Drop` cannot await and a cancellation is a network exchange.

## Decision

Every operation on the `Scanner` trait takes `&mut self`, and starting a scan returns
the GAT `type Job<'a>` borrowing the scanner, so one live job pins the device it runs
on. "One operation at a time per device" is enforced by the borrow checker at every
call site, not by a runtime error.

Cancellation is two-tier:

- `cancel(self)` on the job is the guaranteed path: it consumes the job, performs the
  protocol's cancellation exchange, and reports how the device answered.
- Dropping a job abandons it. The backend performs *lazy* cancellation: the next
  operation on that scanner begins by cleaning up the abandoned device-side job before
  doing its own work. Drop stays cheap and non-blocking; the device is still reclaimed.

The gate is mechanical, not cultural: it is rustc. The borrow checker rejects the
second concurrent job at compile time on every consumer's machine, which is a stronger
gate than anything `xtask` could add; `just placeholder` keeps the cancellation paths
from being stubbed out, and the M1 virtual-device suite (`just test`) carries the
drop-abandonment and cancel scenarios, including against a scripted `hang()`.

## Consequences

The API cannot express "start a job, hand it to another task, keep using the scanner"
without the caller choosing an explicit ownership structure. That friction is the
feature: the structure the caller is forced into is the one the hardware imposes
anyway.

Lazy cancellation means an abandoned job may hold the physical device until the same
`Scanner` value is next used. Callers that drop a job and walk away leave the device
busy for the device's own timeout — accepted, and stated in the trait documentation,
because the alternative (blocking in `Drop`) hangs executors and the other alternative
(a background reaper) smuggles a runtime in against ADR 0004.

The virtual device scripts these exact scenes — abandon then reuse, cancel during a
page, hang during cancel — so the contract is tested from M1 rather than discovered by
users.

The test scenarios were specified at bootstrap, before the trait they exercise
existed.
