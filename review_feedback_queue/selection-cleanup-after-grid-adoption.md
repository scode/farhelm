# Selection cleanup can outlive the xterm surface

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

A queued selection-cleanup callback can throw if the compatibility terminal switches to the grid before that callback
runs. Actual occurrence in a supported browser has not been observed. No lasting input failure, lost work or wider
disruption is established.

## Details

Source coordinates from the original records: `crates/farhelm-ui/assets/terminal.js:4310-4313,6172-6173`;
`crates/farhelm-ui/assets/terminal.js:4312`. References below use these paths' basenames unless another path or revision
is stated.

Recorder disposition: **unverified-after-investigation**. Triage priority: **other**. Source-confirmed conditional
behavior does not imply an observed incident. Reviewer suggestions, narrower refutations and unresolved premises below
remain part of the finding.

Later-turn synthesis after reading g015/f1 and g016/l1's durable records. Both describe the same timer and null
assignment as this ID. The source confirms the conditional defect: terminal.js:4310-4313 queues an untracked
dismissSelection timer; :4298-4300 dereferences mutable outer term; adoptGrid disposes and clears term at :6172-6173
without cancelling that timer. Given input, then adoption, then the timer, the callback necessarily dereferences null.
That conditional conclusion does not establish occurrence or frequency. The exact remaining premise is a supported
browser processing the WebSocket adoption notice between the input task and its zero-delay timer. Neither this pass nor
the peers provide a runtime reproduction. g016's primary-contract inspection reports no universal ordering guarantee
between timer and WebSocket task sources; that supports permission for the ordering, not an observation that an engine
takes it. g015's latest synthesis makes the same distinction. The supported late-notice reveal at :6158-6162 permits
user input before adoption, but alone does not settle task ordering. Runtime experiments remain prohibited. A
deterministic callback-order test can show the conditional failure; observing a real-browser interleaving would
establish occurrence. Cancel or surface-lifetime-guard the sweep.

Suggested bucket: other, unchanged. Confidence: confirmed conditional source defect; actual occurrence/frequency
unverified after investigation. This narrows the earlier confirmed wording and preserves the original finding ID.
proposed_drop: false. possible_cover: duplicate within this review of g015/f1 and g016/l1; commissioner owns
deduplication. No queue coverage checked. Potential highest: no established route. No implied lasting input or
user-owned-work loss.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g014/l1`, `g015/f1`,
`g016/l1`. The retained append-only histories preserve all earlier claims and corrections. Reviewed source references
name their own revisions; the inclusive reviewed tip is the hash above. This review used source inspection only, with no
product runtime experiments or live-install changes.
