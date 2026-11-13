# Grid model

One terminal attachment's committed screen, history, selection and links, built from the supervisor's grid stream. It is
a wire client, not a terminal emulator: the supervisor emulates the terminal, and this crate keeps what it last said. It
has no UI, network or async dependency, so the GPUI terminal draws from it and the tests run without a window.

NOTE: The protocol decoder eagerly decodes each bounded payload. There is no lazy history parser or processing queue
here, and this crate makes no claim about catch-up latency or memory under a flood. Scheduling the work off the UI
thread and measuring real flood behavior belong to the terminal that uses it.

## How it is used

Pass each unchanged binary WebSocket payload to `GridModel::apply`. The public `farhelm_proto::grid` decoder checks
framing and bounds, then `GridStreamCheck` checks ordering before anything changes. A protocol error ends the
attachment; reconnect with a fresh model. Screen rows stay pending until their frame. Read `screen`, `cursor`, `state`
and `take_damage` to draw the latest committed result. OSC 52 clipboard writes are returned from `apply` only when
complete; primary-selection writes are ignored.

The helm's `replay_complete` text control is outside the grid payload format. Call `mark_replay_complete` on that
notice, then use `is_visible` to release the catch-up placeholder. A complete first frame is required. History can be
empty and needs no synthetic append. This flag tracks only the marker; the attachment owner decides other lifecycle
policies that release or replace a placeholder, such as timeouts or failure endings.

History retains at most the advertised cap, with original widths and resolved links. Each message's links are shared
through `Arc<Link>`, so repeated cell references keep one target allocation rather than copying long strings for every
run. A retained row keeps only the targets it uses. `screen_base` and `line` expose absolute line numbers for anchored
scrolling and selection. Clear anchors on `Damage::history_reset`, even when a rebuilt history happens to reuse their
numbers. `selection_text` implements linear and column copy; selection gestures, word expansion, edge scrolling and
clipboard policy outside OSC 52 belong to the UI. `display_text` strips bidi formatting controls for drawing; the stored
and copied text stays unchanged.

## Tests and fixtures

Run `cargo nextest run -p farhelm-grid-model` (or `cargo test -p farhelm-grid-model`). The tests:

- replay every accepted output fixture of the protocol's golden corpus, read in place from
  `crates/farhelm-proto/grid-fixtures/` so the model is checked against the same fixtures the encoder is (the corpus's
  inventory, input fixtures and refusals are `farhelm-proto`'s own tests; a new kind of fixture fails here until it is
  classified);
- replay the old web client's two recorded streams (`tests/fixtures/recorded/`, provenance in
  `tests/fixtures/README.md`);
- check link recognition and disclosure against an oracle generated from the old web client's own JavaScript
  (`tests/fixtures/links-oracle.json`, regenerated with `tools/generate-links.ts`);
- check transitions, copy rules and a valid history append of more than 100,000 rows.

Frame sequencing (rows waiting for their frame, damage, history shifts) is covered by the recorded streams and
`tests/model.rs`; the per-fixture corpus replay checks only that each message's content reaches the model.

## Where it came from

The crate came from the GPUI prototype of Farhelm (Farhelm Light) and was ported to the current grid protocol, which no
longer has terminal kinds. Its model shape is adapted from Farhelm's Apache-2.0 `grid_output/test_client.rs` and the old
web client's `grid-model.js`; copy semantics follow the old client's `grid-buffer.js`, including xterm.js-derived MIT
rules whose notices are in [LICENSE-PORTED](LICENSE-PORTED). No Zed terminal code was copied.
