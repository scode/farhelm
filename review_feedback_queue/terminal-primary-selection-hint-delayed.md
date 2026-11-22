# A checkpoint-delayed primary completion can leave incorrect drag guidance visible for 30 seconds

Reviewed commit: c0827f3edba3dab82228198b4ece283654718800

## TLDR

A checkpoint can delay the terminal's selection notice and leave stale drag guidance visible.

## Details

**other.** Source inspection.

c0827f3edba3dab82228198b4ece283654718800 crates/farhelm-supervisor/src/hosts/terminal.rs:3324-3345,3507-3519,3553-3610;
crates/farhelm-supervisor/src/service/grid_stream.rs:519-562,682-726,790-839; 1a53d9d35889d7dde96e31320cc1681dd013737d
crates/gui/farhelm-gui/src/terminal/attached.rs:1161-1169,3432-3496,3515-3533,3989-3992

If the supervisor recognized a primary-selection write during the drag grace but a checkpoint holds it beyond that
grace, an already-open incorrect hint can stay for its whole 30-second lifetime. The launched snapshot moves the
emulator out before publishing already-fed effects; the grid stream requires a full emulator capture and sends the
primary generation behind grid messages, then the terminal's output handler only increments its write counter. The
1.5-second timer checks that counter once, the shown notice has a separate 30-second expiry, and no late completion
closes it. Once the press for another drag on the same terminal has captured the old counter, the first drag's late
completion can also suppress that later drag's hint if it arrives before the later grace expires. No payload, OS
clipboard change, or input action results from the primary completion. SPEC.md:3438-3449 at
c0827f3edba3dab82228198b4ece283654718800 calls for suppression on a completed program write. SPEC_impl.md:1864-1878 at
the pinned tip documents a 2.235 s median for image-like snapshots. The state transitions are source-supported given the
event order. Incidence of that checkpoint/write/gesture timing in real use is unverified. The reviewer suggested other;
confirmed conditional behavior, , Parent 48dbfa964fce8ac7611d0583ee2136edb66a707d filtered primary writes entirely.
Child c0827f3edba3dab82228198b4ece283654718800 adds the status-generation delivery route without changing the GUI's
handling of late output. Timely primary completions now suppress guidance, while the timing hole remains. Grid stream,
grid model and GUI consumer files are unchanged from that child to pinned tip.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as
`terminal/terminal-primary-notice-backpressure`.
