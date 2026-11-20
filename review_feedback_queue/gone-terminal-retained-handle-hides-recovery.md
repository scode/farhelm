# A terminal Farhelm knows is gone can stay stuck reconnecting

Reviewed commit: `c3569dc51cdab52fd8cd9e7887dcf1e9b67e8718`

## TLDR

If a terminal host ends while its supervisor stays up, the desktop can keep reconnecting to the ended terminal. The
recovery card stays hidden.

## Details

The adopted terminal runtime can confirm permanent loss and retain a Gone handle for the session. Its evidence reports
absence. Session listing tests whether a handle is retained, so the desktop receives a false value for terminal missing
and continues to mount the ordinary terminal area. Publish a confirmed Gone runtime as absent while keeping the separate
unavailable state for a host that may still be alive. Source: `crates/farhelm-supervisor/src/hosts/terminal.rs:840`,
`crates/farhelm-supervisor/src/service/status.rs:339`, and `crates/gui/farhelm-gui/src/session_pane/card.rs:98`.

The listing change landed in `82fb4c7b3` and the recovery card in `fc5ee754c`. Source-confirmed by both assignments. The
review found no additional process loss beyond the terminal host ending.

Provenance: partial trailing review `72ff71f6-2b0b-46a7-946c-7f0293f7890d`, g078/l6, g079/l4, g086/d13.
