# Cancel may erase recovery identity after a permanent Replace answer

Reviewed commit: c1aeb38015b00a8dccc3bfef853f240e512f2e83

## TLDR

If a trusted Cancel reaches the same Replace question after “start, and don't ask again” admits the operation but before
the disabled state takes effect, it can discard recovery identity while replacement continues. A later retry following
partial creation could create another replacement. The required native input ordering is unestablished; this is an
unresolved lead, not a verified reachable bug.

## Details

`crates/farhelm-ui/src/replace.rs:309–340` leaves the permanent-answer question mounted and allows cancellation to clear
its key. `session_view.rs:1549–1575,2683–2690` captures the key and retains the operation claim; Cancel bypasses that
admission. The task proceeds with its captured copy (`replace.rs:417–443`), while another attempt can mint a fresh key
(`179–198,428–433`). Replacement can create the new session then fail to delete the source
(`crates/farhelm-helm/src/sessions.rs:3571–3610,3708–3762`). A fresh-key retry can create another session in that case.
If source deletion completed, the source-existence check at `3498–3519` blocks this route.

Locked Dioxus 0.7.10 separates desktop event dispatch from rendering. Its native interpreter forwards clicks through
synchronous event XHR, while non-headless DOM edits use an animation frame
(`dioxus-interpreter-js/src/ts/native.ts:353–407, 429–493,566–590`; desktop `src/webview.rs:43–99,190–194,537–589`).
Core runtime dispatch has no independent disabled-state veto (`src/runtime.rs:369–379,408–469`). The checked archives
match Cargo.lock, including shipped interpreter JavaScript. That establishes the framework boundary, not native browser
input dispatch or hit testing. **Missing source premise:** a second trusted Cancel delivered after admission and before
the disabled edit. No complete native route was established.

Suggested bucket: **provisionally highest** only for possible loss of user work through duplicate agents; that impact is
also unestablished. The producer suggested high for duplication itself. Confidence: **conditional application sequence,
source-incomplete reachability**. Preserve both uncertainties rather than treating the gap as frequency.

Possible cover: pinned-tip `SPEC.md:858–873,3352–3355` accepts ending a cancelled identity and closely delivered input,
while other rules preserve recovery after unclear outcomes. Independent exclusion judgment was ambiguous.
`TRIAGE_OUTCOMES.md:17373–17394` concerns crossing question openings, not this single opening's admitted key. No exact
other coverage was found. Proposed drop: none.

Provenance: trailing review through `616d79f45d6a1d817833291429d144a5c686bb4b`, private run
`4fae501c-3686-4822-aa6c-037266637632`, resumed `legacy-g125/discovery-f1`, fresh `a010/f1`. Both investigations stop
incomplete on the stated source premise. No runtime reproduction.
