# The steady-block documentation fix left the sender's older comment intact

Reviewed commit: 7b2f02b18b7c44f329bd8270399f64427b435343

## TLDR

The terminal sender's comment describes a blinking cursor even though the updated contract specifies steady block
behavior.

## Details

**other.** Source inspection.

7b2f02b18b7c44f329bd8270399f64427b435343 crates/farhelm-proto/src/grid.rs:185-189;
crates/farhelm-supervisor/src/grid_output.rs:340-354; crates/farhelm-supervisor/src/grid_output/tests.rs:553-573

The shipped GUI still shows an explicitly steady block as blinking. This is deliberate behavior documented by the child,
and the remaining review item is a stale source comment. Parent c0827f3edba3dab82228198b4ece283654718800 and child
7b2f02b18b7c44f329bd8270399f64427b435343 have the same sender at grid_output.rs:340-354, including the old 'unless the
program says otherwise' comment. The child corrects the wire prose and the cursor test's claim, with no production
change. The source route is concrete: vendor/vte/src/ansi.rs:868-880,1778-1794 maps the steady block to default-valued
CursorStyle. vendor/alacritty_terminal/src/term/mod.rs:1018-1029 returns the value without origin information. The
sender treats the default value as blinking. At pinned tip 1a53d9d35889d7dde96e31320cc1681dd013737d these cursor paths
are unchanged. The current cursor test covers default blink and other shapes but does not pin the documented exception.
The reviewer suggested other, confirmed documentation follow-up, , Possible coverage: true.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as
`terminal/terminal-steady-block-contract`.
