# TRUST_RETRY and redraw detection can resend printable 'y' into Grok's composer before type_text (unverified premise)

Reviewed commit: 1b3fe49af86979aebc2719965682a17f8c6366ac

## TLDR

In the Grok drift test, if Grok's folder-trust dialog takes longer than 3 seconds to dismiss or redraws while
dismissing, the harness resends the printable character `y`. That extra `y` could land in Grok's input composer ahead of
the test prompt and be submitted with it without the echo check noticing.

## Details

**other.** Unverified after investigation. Source inspection. Main reference
`crates/farhelm/tests/e2e/real_agent_capture/drift.rs:267`.

(1) Source basis: In `crates/farhelm/tests/e2e/real_agent_capture/drift.rs:267-273` (`trust_answer`, added in
`f2587333`), Grok's trust menu is answered with printable `b"y"` rather than a control key (`\r` or `\x1b[B`). In
`Terminal::ready` (`drift.rs:401-416`), `unanswered` becomes true whenever
`*screen != rendered || at.elapsed() >= TRUST_RETRY` (`TRUST_RETRY` is 3 seconds at `drift.rs:21`, added in `df89423e`),
resending `b"y"` on any 350ms `pace()` tick whose rendered screen differs or after 3 seconds while `trust_prompt(&text)`
still matches. Subsequently `execute` calls `first.type_text(&prompt, false)` (`drift.rs:694`) and sends `b"\r"`
(`drift.rs:695`), where `type_text` (`drift.rs:431-439`) checks only `text.contains(value)`. (2) Suspected consequence:
Unlike a stray `\r` in an empty composer, a resent printable `b"y"` queued in the PTY after Grok already accepted the
first `b"y"` would be inserted into Grok's composer before `prompt`, producing `"yRemember test label ..."` (which still
satisfies `text.contains(value)`) and submitting a corrupted prompt—or triggering an unintended single-key action during
TUI transition—causing a spurious `drift_grok` failure. (3) Checks actually performed and their results: Inspected
`df89423e5444db34f4a96d684c01870f04aceb74`, `f2587333c30bd70cd8b5bf77424091d8a5b1e6a8`, and
`crates/farhelm/tests/e2e/real_agent_capture/drift.rs:21, 263-286, 401-445, 683-695` at
`1b3fe49af86979aebc2719965682a17f8c6366ac`. Confirmed from source that `ready` has no special handling for printable
trust answers, resends on both redraw (`*screen != rendered`) and `TRUST_RETRY` (3s), and `type_text` does not clear the
composer or require `value` to be the entire composer buffer. No live Grok CLI was executed. (4) Exact missing premise:
Whether live `grok` (Build 1.0.92+) can take >3 seconds to dismiss its folder-trust screen or emit an intermediate
redraw that still matches `trust_prompt(&text)` after accepting the first `y`, AND whether Grok's composer reads and
retains buffered PTY bytes received prior to mounting the composer rather than flushing/discarding them. (5) Why
available source checks cannot resolve it: `grok` is an external closed-source vendor CLI whose terminal input
buffering, trust-dialog dismissal latency, and intermediate screen redraws are not present in this repository's source
tree. (6) What evidence would settle it: Observing a real `grok` TUI session (e.g., via `drift_grok` under slow
startup/load or a PTY transcript capture) showing either that buffered `y` bytes sent during trust dismissal appear in
the composer/corrupt the prompt, or that Grok flushes/ignores extra input across the trust-to-composer transition and
never redraws or exceeds 3s.

Fix (if triggered): clear pre-existing composer input before `type_text` or avoid aggressive redraw/3s resends for
printable single-key trust answers.

Reviewer metadata (g13_drift_private_hooks/f5): suggested bucket: other — confined to the opt-in maintainer drift test
harness (`drift_grok`).; confidence: unverified after investigation (source basis:
`drift.rs:21, 267-273, 401-439, 694-695`; suspected consequence: stray `'y'` prepended to Grok prompt or stray TUI key
on slow/redrawing trust dismissal; checks performed: source inspection of `df89423e`, `f2587333`, and `drift.rs` at
`1b3fe49a`; missing premise: live Grok trust-dismissal timing/redraws and pre-composer PTY input buffering; why
unavailable: external vendor CLI behavior not in repo source; settling evidence: live Grok PTY trace/observation during
trust dismissal).; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g13_drift_private_hooks/f5`.
