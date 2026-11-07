# Resume test does not force its independent connection boundary

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

The regression test never forces a ready checkout preview while the form still displays Connecting. A regression at that
boundary can consume the single accepted launch press without eventually starting a session.

## Details

Source coordinates from the original records:
`e2e/tests/tensorlake-lifecycle.spec.ts:948-953; crates/farhelm-ui/src/list/create_form.rs:6393-6415`. References below
use these paths' basenames unless another path or revision is stated.

Recorder disposition: **verified**. Triage priority: **other**. Source-confirmed conditional behavior does not imply an
observed incident. Reviewer suggestions, narrower refutations and unresolved premises below remain part of the finding.

Later-turn synthesis rechecked the actual fixture and downstream guard. e2e/tests/tensorlake-lifecycle.spec.ts:61-64
gives Connecting the real fresh connection token; 798-800 holds a response whose JSON was captured as Connecting at
55-75. The independent Resume helper can read Connected after 802-803 while that displayed read stays held. A fresh
preview is separately held at 862-864. The zero-create assertion at 950-951 is still before that preview is fulfilled;
952 releases lag first, then 953 releases previewGate. This does not force PreviewState::Ready with the form's
live_resume_display still identifying Connecting. The production observation is synchronously derived from the rendered
host snapshot (create_form.rs:255-263,3203-3206), and the explicit phase wait is 6393-6397.

Preserved correction: removing that phase wait does not establish an early-create consequence. The effect clears
guarded_submit_pending before ordinary submit (6414-6415); the latter refuses Connecting at 5313-5320 because
HostOption::available_for_launch(None) at shared.rs:398-400 rejects a nonempty phase that is not Suspended. It can
therefore spend the press and never launch after the host read catches up. If the released host read renders first, the
existing test can pass without the wait, so final one-create checks do not independently establish the adverse ordering.
No mutation execution was run or claimed.

Strengthen the test by releasing/observing the fresh Ready preview while retaining the displayed Connecting read,
asserting no dispatch at that settled preview boundary, then releasing the host read and requiring exactly one eventual
create. That final create proves the accepted press survived the refusal-prone interval. Keep a separate held-preview
assertion. The requirement is specifically this fresh-checkout effect, not the existing-folder helper path, which
legitimately launches from its private Connected read. SPEC_impl.md Tensorlake hosts:6188-6196 defines the form-owned
one-shot/current-connection preview authority; the source's 6393-6397 comments require waiting for the form to observe
Connected.

Suggested bucket: other (test evidence gap only). Confidence: confirmed by fixture gate order and downstream source,
with the consequence corrected to a potentially spent press. proposed_drop: false. possible_cover: none.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g006/g006-l5`. The
retained append-only histories preserve all earlier claims and corrections. Reviewed source references name their own
revisions; the inclusive reviewed tip is the hash above. This review used source inspection only, with no product
runtime experiments or live-install changes.
