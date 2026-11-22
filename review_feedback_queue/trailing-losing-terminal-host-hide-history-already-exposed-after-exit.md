# Losing the terminal host can hide history already exposed after exit

Reviewed commit: 0592c5c6d667abe2f08706185b7566febb3434f7

## TLDR

During recovery from loss of an already exited terminal host, the retained screen can lose its exit line and stop
showing primary history even though recovery has kept the same grid.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-grid-model/src/lib.rs:472`.

hosts/terminal.rs 475–515 and 4088–4148 permit Exited to move to Gone or Unavailable on host connection failure.
Set_liveness 4033–4047 changes status and try_read 2199–2211 allows a reader of the unchanged resident emulator.
grid_stream.rs 526–534 reads the changed status without requiring the emulator to be replaced, 693–708 captures modes,
754–757 and 797–800 send a status-derived State before live handles its end (604–626). with_exit at 909–920 clears exit
for both ended-runtime statuses. The last cycle can therefore send exit None on the same attachment while alt_screen
remains set. AttachedTerminal applies output before the stream-state event and keeps its committed grid when the stream
stops (attached.rs 1096–1119, 1241–1254). hides_primary_history at grid-model lib.rs 470–475 and viewport first_content
61–74 then treat the unchanged alternate grid as live, and attached.rs 2117–2128 drops the exit line. Viewport sync at
48–55 still sees the same alt mode and does not retire its former history anchor or selection. This is not a normal
Restart, which replaces the attachment. Later 063203912059cb23101d9a458d8821f02f1bc18f marks genuinely Gone as missing,
allowing the app to replace that pane with its recovery card (supervisor service/status.rs 319–326; GUI
session_pane/card.rs 98–106). It deliberately leaves Unavailable clear, so that branch can still keep the affected
snapshot while re-adopting. SPEC.md tip 3301–3304 and 3467–3470 define history browsing and pointer ownership for an
exited terminal, and SPEC_impl.md 564–567/2361–2365 describes retaining or recovering the prior frame. Keep the prior
exit semantics for the retained grid until the viewer replaces that attachment, or carry terminal availability
separately on the wire. Bucket other: affected state is a recovery snapshot, with no source evidence of permanent
history loss. Confidence confirmed by source for final-cycle exit clearing when the emulator is readable and the cycle
is delivered. Whether a distinct native frame is presented before a concurrent card/replacement is unverified. Possible
coverage recovery group (unavailable host and retained frame), matching left to commissioner. Coverage:
grid_stream/tests.rs 932–943 asserts that child exit leaves the stream up, and 953–972 follows an unreadable host from a
fresh attachment and inspects the stream end. They do not combine exit, retained alternate history, and host loss or
assert the final wire State. The added recovery assertion at service/status.rs 869–881 checks Gone/Unavailable/Exited
listing classification individually and does not exercise a retained GUI snapshot. The existing GUI exit-clear assertion
at attached/tests.rs 1298–1330 uses a synthetic state update with alt_screen false. No later reviewed runtime/test
change closes this composition gap.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `scrollback/l8`.
