# The baseline test uses a state that never cues

Reviewed commit: 806ac6bcbe07b9f9cbb7577647d9ec5a5217f729

## TLDR

The test can pass if a reconnect incorrectly animates existing warnings. Its first snapshot contains a host state that
never produces a cue.

## Details

`crates/gui/farhelm-gui/src/session_list/attention/tests.rs:47–110` seeds Unreachable for its initial suppression check.
The selector at `crates/gui/farhelm-gui/src/session_list/attention.rs:302–305` cannot produce a cue for that state on
any snapshot. The later retry assertion also gives a SignInNeeded host an idle projection. Start a separate baseline
with a cue-eligible reason and use the real sign-in projection for the retry. The assertion gaps follow from source.

Suggested bucket: other. Source confidence and remaining conditions are described above.

Provenance: trailing review 116de16d-0880-420e-9e7e-87f8b9869bf3, `attention-cues/a5`, `sign-in-contract/f4`.
