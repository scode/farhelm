## What this was about

This plan handles 21 terminal review outcomes. Restart could keep waiting for scrollback after the old terminal ended. A
terminal program copying to the primary selection could still trigger drag-copy guidance. Attachment diagnostics could
hide part of a session ID, and terminal links could keep more memory than their budget counted. The rest of the review
covered terminal contracts and the evidence behind terminal tests.

## Things you should know

If a terminal ends before it can supply its scrollback, Restart can continue with an empty terminal. On normal
termination, callers learn when their pending input went unsent. A primary-selection copy now suppresses drag guidance
without changing what is on the system clipboard, even when the program sends many selection updates. The completion
uses the existing grid message. The only change in the desktop test area supplies empty selection text to the existing
test of the notice's grace period.

A remote Mac update can replace the executable before the old supervisor restarts. The terminal host's startup contract
now describes the compatibility needed in that window. The grid docs also explain why an explicit steady block can show
as a blinking block.

A reflow timeout now leaves a named failure in the test log and exits the test process before shrinking can start more
workers. When macOS truncates a process listing and cleanup cannot tell whether a test host is gone, cleanup keeps the
run directory and socket for diagnosis and retry.

## Open questions and possible follow-ups

Two rendering fixes are in Near term under Plan follow-up. A wide glyph that wraps can leave the final column with the
wrong background. Erasing only its right column can hide the new background there. In both cases the viewer currently
uses the shape of the rows to tell a wide-wrap placeholder from copied text. Either small paint-only fix can put an
unwanted space into copied paths or split links across the wrap. The follow-up should carry that distinction to the
viewer and check paint, selection, paths and links together. This plan can be approved with both fixes tracked there.

The stalled-input test checks for a live child and discards reported by the host. An optional follow-up could add a
fixture that witnesses the pending bytes and can detect a silent drop.

## Pull requests

- Deferred both wide-cell background findings to the two follow-up entries.
  [#4472](https://github.com/scode/farhelm/pull/4472/changes)
- Fixed the hyperlink memory budget by releasing spare URI capacity before retaining an admitted link.
  [#4474](https://github.com/scode/farhelm/pull/4474/changes)
- Fixed session-ID escaping in attachment logs, corrected the input-test comments about closure and frame delivery, and
  fixed the detach test to verify the writer fills and the caller waits out its grace.
  [#4478](https://github.com/scode/farhelm/pull/4478/changes)
- Fixed late terminal requests at retirement, including the scrollback reply that Restart waits for.
  [#4480](https://github.com/scode/farhelm/pull/4480/changes)
- Fixed drag guidance after a primary-selection copy by forwarding completion without the text.
  [#4484](https://github.com/scode/farhelm/pull/4484/changes)
- Corrected the steady-block cursor and remote Mac startup documentation.
  [#4485](https://github.com/scode/farhelm/pull/4485/changes)
- Strengthened the stalled-input, pre-client output and synchronized-update tests. The Shift+Enter integration test
  checks for both bytes in the real program's first read. The deterministic host test checks the single write offer.
  Fixed reflow timeout containment and preservation of incomplete Mac test runs.
  [#4487](https://github.com/scode/farhelm/pull/4487/changes)
- Strengthened the refused-input, paste receipt, oversized paste and emulator-lock tests, and fixed lost last-frame
  timing in the resize measurement. [#4489](https://github.com/scode/farhelm/pull/4489/changes)

## Checks run, reused and skipped

- `cargo test --manifest-path vendor/vte/Cargo.toml --features ansi` passed 62 tests
  (`ac63e0d0-5cc7-4b31-bf52-a4019607bbb6`).
  `cargo test --locked --lib --manifest-path vendor/alacritty_terminal/Cargo.toml` passed 166
  (`b1207045-c126-4dcd-b98b-1a88857dc7ca`), and `cargo test --locked --manifest-path vendor/pair-tests/Cargo.toml`
  passed three (`ecb38802-a68c-4eee-8889-4e540a11edd3`). The patched terminal tree matched a clean reproduction from the
  published crate. `cargo nextest run -p farhelm-terminal-emulator` passed 118 tests
  (`042aa081-963f-4d5a-8591-8c4a5dfe62b0`). The ordinary selection omits 20 opt-in cases for extended workloads,
  measurements, private recordings and fixture generation. The vte result covers the final patch because its standalone
  source and configuration stayed the same.
- `cargo nextest run -p farhelm-helm --lib -E 'test(/^terminal::tests::/) | test(/^client::tests::/)'` passed 97 tests
  (`70603eba-eb90-4d33-9dba-f34620724b12`). A single-test selection of the final teardown grace assertion also passed
  (`440ec35e-41d8-4d00-adc7-523b3566571d`).
- `cargo nextest run -p farhelm-gui --lib -E 'test(a_program_clipboard_write_in_the_grace_window_suppresses_the_notice)'`
  passed with the empty primary-selection message (`b3b19009-c43f-4b30-916f-aab58bc3da7d`). The terminal-host, narrow
  emulator and synchronized-update tests passed 126 selected cases (`334263c5-afc0-4de4-96b6-baa154bbc1e7`) with
  `cargo nextest run -p terminal-host -p farhelm-terminal-emulator -E 'package(=terminal-host) | binary(=narrow) | binary(=sync)'`.
  The opt-in fixture writer was skipped because the golden data stayed the same.
- On the rebased tip,
  `cargo nextest run -p farhelm --test terminal_host_client -E 'test(=shift_enter_reaches_the_program_in_one_read)'`
  passed the exact real-program test (`2609a7e7-89e5-4490-b0f0-db114d47bb90`). The filter omitted ten other client
  tests.
- `cargo nextest run -p farhelm-supervisor --lib` passed 149 tests with the following filter
  (`ba9e28b7-2162-40ef-ae65-e45134dc1970`).
  `test(/^hosts::(terminal|client)::/) | test(/a_restart_seed_|_seed_of_an?_|a_seed_whose_runtime|a_stop_during_a_restarts_seed/) | test(/^grid_output::/) | test(/^service::grid_stream::/) | test(/^agent_kind::(screen_fixtures|status_line_fixtures)::/)`
  The initial run retained one fixture failure (`d464a1a6-8c45-44d1-bf55-458c4e9adbcb`). Its stand-in counted input
  cumulatively, while the test expected only its second marker. The corrected exact test passed
  (`433947bb-9ebc-4574-97d6-003a58e7b844`) before the combined run. The typical resize measurement selected with
  `--run-ignored only --no-capture` in the debug build carried 1,995,689 bytes and reported a 3,698 ms last arrival
  (`758e3330-ac9a-4a74-8cb9-d2db61c2bbbb`).
- At the rebased tip, Rust formatting, the test-delay checker, dprint on changed files, changelog format, whitespace and
  instruction-reference checks passed. The instruction-reference checker reported one existing historical archive
  citation to a missing heading. All 21 archive moves were checked against the corresponding PR parent.
- Reused the passing results covering the changes at pre-rebase tip `28920459a8a65437e586dd0be5010fd3f291bb8c`. The
  source, tests, vendor patch and fragments matched that diff byte for byte. Earlier focused runs still apply where
  subsequent edits touched only other parts of the stack or clarified comments and diagnostics. Main's changes corrected
  adjacent spec and source comments, graduated other findings and added a subsection for plan follow-ups. The two
  terminal TODOs moved there.
- Validation used the selected vendor, emulator, helm, desktop and terminal-host suites on Linux. The Mac process-list
  branch had source review. The behavior changes are covered by the focused checks, so the release battery and host
  mutation or coverage runs were skipped. The documentation-only cursor and startup PR used static checks.

## Review gate

Every PR passed the plan's independent source review and cold wording review. Review exposed the shared wide-wrap
representation issue, a possible discarded Restart reply, clipboard ordering, and test premises that needed stronger
evidence. Those findings drove the deferrals and corrections above. The report received a separate cold read.
