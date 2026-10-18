# A refresh can remove the warning for bytes already copied

Reviewed commit: 5e1f1cfffe38f41e450ab70d440ca8fabac8be36

## TLDR

A session refresh can remove the hidden-character warning after copying a command or folder, while the clipboard still
contains the original bytes and the copied indicator remains visible.

## Details

`copy_value` copies its captured value but keeps only a click counter for feedback
(`crates/farhelm-ui/src/session_view.rs:2144–2158`). Rendering recomputes the warning from the current
`shown.cwd`/`shown.invocation` (`2284–2287`). If accepted metadata replaces the field with safe text during the
1.5-second feedback interval, the warning describes that newer value rather than the bytes copied. Capture warning state
with the click so its lifetime follows the copied value.

Suggested bucket: **highest provisionally**, solely because a lost disclosure might contribute to harmful subsequent
paste/use; that security consequence has not been verified. If the consequence is solely a transient disclosure error,
the reviewer suggested **other**. Confidence: **confirmed static state/data mismatch**; harmful paste or execution,
malicious scheduling and omission of the first warning paint are unverified. A legitimate Restart with can also change
the invocation. No runtime tests ran.

Possible cover: `SPEC.md:2870–2875` explicitly allows remote programs to replace the clipboard through OSC 52; this
finding establishes no new clipboard-write authority. `TRIAGE_OUTCOMES.md`, `session-header-raw-peer-text.md`
(`2287–2310`), separately requires a hidden-character warning while intentionally preserving raw copied bytes. It does
not accept refreshing that disclosure away. The normal feedback expiry in `SPEC.md:1631–1632` does not clearly accept
warning about a different value before expiry. No matching Planned, BUGS or current queue entry was found.

Proposed drop: the originating reviewer conditionally proposed FILTER.md's **Rare, self-correcting glitches and
imprecise diagnostics**, only if the lost disclosure has no security consequence. Independent drop-review-5's final
verdict is **KEEP**: allowed clipboard writing does not waive the separate header-copy explanation or establish that
misleading feedback has no additional security consequence. Uncertainty prevents applying the diagnostic filter.

Provenance: persistent Astra/high review, `ui-session-detail-1/f1`; independent drop-review-5 final KEEP, item 3.
