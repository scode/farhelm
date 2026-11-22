# The Shift+Enter integration assertion is stronger than the first-write contract

Reviewed commit: aecc8f8ece9a633e63f6cbca8e0edb6d1185ee35

## TLDR

The Shift+Enter integration check assumes the program receives the key in one read, which the terminal host contract
leaves to the PTY and program.

## Details

**other.** Source inspection.

aecc8f8ece9a633e63f6cbca8e0edb6d1185ee35 crates/farhelm/tests/terminal_host_client.rs:683-706;
crates/terminal-host/src/host.rs:570-619,4538-4569

This is a test contract mismatch. The production host offers a fresh input segment whole to the PTY and keeps any
unwritten suffix on a short write, as SPEC.md:3228-3235 and crates/terminal-host/SPEC.md:387-401 require. A separate
write-observing unit test checks that host-side boundary, while the integration fixture injects the two bytes directly
and asks dd for one block, so it neither measures first-write atomicity nor exercises GUI chord encoding. The 200 ms
sleep does not prove the program has entered read. Parent 7b2f02b18b7c44f329bd8270399f64427b435343 and child
aecc8f8ece9a633e63f6cbca8e0edb6d1185ee35 run the same fixture and assertion, the child changing comments only.
Conditional consequence is a false failure if the test PTY produces a different read boundary. No actual split under the
fixture's settings or shipped regression was established. The reviewer suggested other, confidence confirmed for the
test expectation, , Possible coverage: true.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as
`terminal/terminal-shift-enter-read-boundary`.
