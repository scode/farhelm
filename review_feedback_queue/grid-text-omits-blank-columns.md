# Grid text helper loses spaces omitted from wire runs

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

The new test helper can turn visibly separated terminal text into a different string. A test searching that transcript
for the displayed text may then time out or match text that was never contiguous on screen. No currently failing
assertion was identified.

## Details

Source coordinates from the original records: `crates/farhelm/tests/e2e/harness.rs:177-200`;
`crates/farhelm/tests/e2e/harness.rs:177-200; crates/farhelm/tests/e2e/terminal_tabs.rs:314-395`. References below use
these paths' basenames unless another path or revision is stated.

Recorder disposition: **verified**. Triage priority: **other**. Source-confirmed conditional behavior does not imply an
observed incident. Reviewer suggestions, narrower refutations and unresolved premises below remain part of the finding.

This is a whitespace reconstruction defect, independent of known repeated-row/temporal-order findings. grid_text's
row_text closure at harness.rs:179-181 concatenates run.text and ignores run.col and run.width. The actual protocol
contract at proto/grid.rs:96-116 says columns no run covers are default-style blanks. grid_output.rs:134-142 and
:215-232 deliberately trim default-style spaces at both ends of ASCII runs, adjusting col, and omit all-blank runs. Thus
a styled run followed by default-style ' starting' is encoded with a one-column gap; concatenation removes the gap. The
current record fixture prints colored 'fake-agent', resets style, then ' starting (script=record)' at
fake_agent.rs:961-966, so its visible startup text is flattened as 'fake-agentstarting (script=record)'. Likewise the
plain '> ' prompt at fake_agent.rs:1021-1023 loses its trailing blank, contrary to harness.rs:173-174's explicit
preservation claim. The current prompt-sensitive reattach consumer is deliberately harness_tmux
(session_lifecycle.rs:491-522), so do NOT claim that test fails today. The ordinary Unicode probe in
prompt_running_agent uses a byte-count/file witness, not this full text needle; no false-pass claim is made for it.

Reconstruct blank gaps up to each run.col using cell widths; for trailing padding choose/document a consistent row-width
policy, since the wire omits default padding. The issue is source-verified for real emitted rows, with no runtime
experiment required or performed. Existing plain-ASCII marker callers usually avoid the missing-gap shape, so immediate
suite impact is limited to the false shared-helper contract and text representation; no product terminal bug is
asserted.

Suggested bucket: other. Confidence: confirmed transform defect, no failing current assertion established.
proposed_drop: false (commissioner decides whether the helper-contract defect merits recording despite limited current
caller impact). possible_cover: none; g040/l6 checked a different padding premise and correctly says default blanks are
omitted. Could be highest: no, test helper only.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g041/g041-d1`,
`g046/g041-d1`. The retained append-only histories preserve all earlier claims and corrections. Reviewed source
references name their own revisions; the inclusive reviewed tip is the hash above. This review used source inspection
only, with no product runtime experiments or live-install changes.
