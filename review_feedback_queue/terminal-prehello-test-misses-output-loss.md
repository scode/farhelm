# The corrected pre-Hello fixture still checks only a substring before exit

Reviewed commit: aecc8f8ece9a633e63f6cbca8e0edb6d1185ee35

## TLDR

The terminal lifecycle test checks a substring and can pass when other expected output is missing at exit.

## Details

**other.** Source inspection.

aecc8f8ece9a633e63f6cbca8e0edb6d1185ee35 crates/terminal-host/tests/host_lifecycle.rs:17-58;
crates/terminal-host/tests/common/mod.rs:188-215,350-423; crates/terminal-host/src/host.rs:347-356,454-526,625-651

The revised test now establishes that the child wrote and exited before protocol admission, but it does not verify exact
output through the exit record. Parent 7b2f02b18b7c44f329bd8270399f64427b435343 completed Hello in its wait. The child
aecc8f8ece9a633e63f6cbca8e0edb6d1185ee35 waits via a marker written after the PTY write and a child-exit check. Its
fixture's socket peer-credential probe does not send Hello. Both revisions stop reading output on the same substring,
then skip non-exit records. A missing newline or a trailing fragment ordered after exit would evade that assertion. The
production host reads the PTY without requiring a client, journals available bytes, drains within the stated bounds
before recording exit and refills a client's stream from its journal cursor. This conforms to the relevant ordering in
crates/terminal-host/SPEC.md:128-145,168-189,209-225. No shipped loss or reordering is established. The reviewer
suggested other, confidence confirmed for the test gap, ,

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as
`terminal/terminal-prehello-output-oracle`.
