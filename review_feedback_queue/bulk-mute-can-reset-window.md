# Bulk Mute can reset the window before all updates arrive

Reviewed commit: e7587f3b2478cf6f42416b3b631d5fc667f5b148

## TLDR

A large marked selection can reset the desktop connection when you choose Mute or Unmute. Farhelm can apply only part of
the batch and lose track of which updates finished.

## Details

The bulk action starts one command per eligible row without backpressure. The desktop sends them through the ordinary
command channel. The helm handles one command at a time and closes the connection if its waiting queue fills. The client
then discards outstanding reply tracking. The source establishes the conditional failure when an ordinary batch arrives
faster than the helm handles it. Scheduling and frequency were unmeasured.

At `c3569dc51cdab52fd8cd9e7887dcf1e9b67e8718`, see `crates/gui/farhelm-gui/src/session_list/marking.rs:897`,
`crates/farhelm-helm/src/ui_channel.rs:996` and `crates/farhelm-helm-client/src/channel.rs:534`. SPEC.md describes
eligible per-row metadata writes and fleets of a few hundred listed rows under Session list. Pace or batch the ordinary
writes while retaining feedback for each target.

Suggested bucket **high**. Source-confirmed, from trailing reviewer g044/l8.

Provenance: partial trailing review `72ff71f6-2b0b-46a7-946c-7f0293f7890d`, g044/l8.
