# A queued launch can start a different template

Reviewed commit: 3446cd696290ac710c9d0e38d970cc99f61b3253

## TLDR

A launch sequence can show one template and then start another after its shortcut changes in a different client. The
second template may use a different host or folder.

## Details

The sequence stores digit and bolt references, while its status pill names them from this client's preference snapshot
(`crates/farhelm-ui/src/launch_sequence.rs:9–55`). Enter resolves those references with a fresh helm read and directly
launches the result (`crates/farhelm-ui/src/list/view.rs:3411–3414,3429–3449,3543–3550`). A person can queue key 1 while
the pill says template A, another client can bind key 1 to runnable template B, and Enter can start B while A remains on
screen. The sequence has no timeout. A two-client reproduction would confirm the rendering and launch sequence; the
static state flow is clear.

Suggested bucket: highest, for an unintended launch on a different host or in a different folder. Confidence: likely; no
runtime reproduction or harmful launch was performed. Provenance: trailing review of the stated commit. `SPEC_impl.md`,
“Launch templates,” specifies late reference resolution but does not accept a mismatch between the displayed name and
the launched template. The earlier triage decision about a brief local save race does not cover an indefinitely armed
sequence. A fix should keep the displayed choice and the choice used by Enter together, or update the pill before launch
when a binding changes.
