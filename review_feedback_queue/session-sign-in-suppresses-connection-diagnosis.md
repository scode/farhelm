# Host sign-in can hide the session’s connection diagnosis

Reviewed commit: fc361c4ba6a00ebe69be51fc81fe07ae034a1959

## TLDR

A session can say to answer a sign-in question to reconnect even when the host still needs an update or another Farhelm
manages it.

## Details

The unavailable card replaces its main diagnosis and remedy at
`crates/gui/farhelm-gui/src/session_pane/unavailable.rs:380–397` and `:449–472`. After maintenance questions were added,
`crates/gui/farhelm-gui/src/sign_in.rs:49–77` selects without filtering for the Sessions purpose and prefers Setup. The
card’s smaller detail can still show the underlying phase. Keep the session diagnosis and its remedy, and only present
an answer as a route to reconnecting when it belongs to the session connection. Source-confirmed presentation logic.
Runtime incidence is unmeasured.

Suggested bucket: other. Source confidence and remaining conditions are described above.

Provenance: trailing review 116de16d-0880-420e-9e7e-87f8b9869bf3, `host-attention/l3`, `host-attention/l9`.
