# What three review rounds found in M1, and what that says about the risky parts

Written after M1 landed, as a record of which parts of this design turned out to be dangerous in practice. The swarm ran
three rounds over the walking skeleton, then a documentation pass, then a documentation-correctness round.

The interesting result is where the bugs clustered. Almost none were in the ordinary Rust — they were in the seams
between Farhelm and the things it drives, and in fixes made by earlier rounds.

## The tmux seam produced the worst bugs

Every one of these was invisible to a passing test suite and would have looked like a mysterious terminal glitch to a
user:

- The control-mode reader decoded UTF-8 lines. tmux passes bytes ≥ 0x80 through raw, so `cat` of a binary file killed
  the terminal permanently. Real Claude Code masked this because its box-drawing is valid UTF-8.
- `capture-pane` newline-terminates its last row too, so replaying it verbatim scrolled one row past the content — a
  cursor one row low on the normal screen, and a destroyed top row on the alternate screen.
- The alternate-screen switch was emitted after the content prefill. `\x1b[?1049h` clears the buffer it switches to, so
  reattaching to a full-screen agent showed a blank screen.
- Pane-mode formats were space-separated and parsed with `split_whitespace`, so an unknown format name (tmux expands it
  empty) collapsed the field and shifted every later value one position left.
- `bracket_paste_flag` arrived in tmux 3.7, not the 3.3 floor the design assumed. Ubuntu 24.04 ships 3.4.
- `send-keys -H` rejects ~1000 arguments as "command too long", which a well-meaning chunk-size increase discovered the
  hard way.

The lesson is narrow and useful: assumptions about tmux were wrong roughly as often as they were right, and the only
thing that caught them was running tmux and looking. Reviewers that probed a private socket found real defects;
reasoning from memory produced confident wrong claims in both directions — one reviewer asserted a "two control clients
receive no output" mechanism that a later reviewer disproved empirically.

## Fixes introduced their own bugs at a high rate

Round 2 and round 3 each found defects in the previous round's fixes:

- A liveness check compared channel ids, which are only unique per connection.
- A 1 MiB WebSocket cap would have dropped the connection on exactly the large paste that chunking existed to support.
- A tmux capability probe ran with no target immediately after `start-server`, when no session exists — so it warned on
  every start of a healthy tmux and stayed silent on genuinely old ones. Precisely inverted.
- `%begin` was treated as "attached" when it only opens the reply block; a failed attach would have been reported as
  success with tmux's diagnostic discarded.

Worth remembering when deciding whether a round is worth running: the marginal round found fewer but not less serious
problems, and the ones it found were concentrated in code written days, not months, earlier.

## Flaky tests were pointing at a real bug

The takeover test failed intermittently under load. The temptation was to raise the timeout. The actual cause was that
`open_output_stream` returned before the control-mode client had attached, so output produced in that window was lost —
which for a user means keystrokes that vanish right after reattaching. Bounding harness concurrency and waiting for the
attach made the test deterministic and fixed the product.

The one test that stayed flaky after that was asserting on terminal output at process death, which races teardown by
construction. Polling tmux for `pane_dead` instead is both deterministic and a more honest statement of what the test is
about.
