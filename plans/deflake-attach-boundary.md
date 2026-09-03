# Plan: make the Rust e2e harness indifferent to the attach race

Anchor: commit d8cd8082aff0, 2026-09-03. Serves the TODO.md "Systematic deflake" entry "Make the e2e harness indifferent
to the attach race". Before executing, diff `d8cd8082aff0..main` over the paths in "Files touched" below and re-read
FLAKES.md for any new entry naming the attach seam; see AGENTS.md's `plans/` section for the staleness rule.

NOTE: This is a test-harness change only. No product code moves. It does not un-ignore the four tests the 0.3.0 release
gate ignored; those sit on other seams (fixed time budgets, the launch-shim race) and have their own TODO entries.

## The problem, in one paragraph

Attaching to a session (`SupervisorClient::attach`, farhelm-helm's client.rs) delivers a `TermStream` whose `Data`
events carry first the supervisor's catch-up replay of the pane — tmux's `capture-pane -e` rendering of the screen: rows
padded with spaces to the pane width, cursor-address sequences, invalid bytes canonicalized — and then the live pty
bytes, on one queue, with a `TermEvent::ReplayComplete` between them. A fixture that prints something at startup gets it
captured into that snapshot whenever it wins the race against the test's attach. On a developer machine the attach wins;
on a loaded 4-vCPU box the fixture wins, and the same output arrives in the snapshot shape. Four tests were fixed one at
a time for this during the 0.3.0 cut (FLAKES.md, entries dated 2026-09-02 and 2026-09-03 for hexecho, non_utf8, and the
two argv-width guards), each growing a private helper that the next test then needed again. The harness has no shared
primitive for either "the shape does not matter" or "only the live bytes matter", so every test rediscovers the seam
under load.

## Decisions (maintainer, 2026-09-03)

- Add a live-boundary primitive on top of `ReplayComplete`, so tests that make claims about the LIVE stream can wait on
  the live segment only, instead of stripping their way around the snapshot.
- Startup lines that tests read (the `FAKE-AGENT ARGV:` row, PID rows) stay printed at startup and are normalized on
  read. Print-on-request stays reserved for byte-fidelity fixtures, where normalization cannot help (the `binary` script
  already does this); no further fixture changes.
- Consolidation is minimal: the shared normalizer, one shared argv-marker reader instead of two, and the hexecho
  tokenizer folded onto the normalizer. `counter_records`, `flood_records`, `wait_for_bytes`, and the three
  attach-then-READY helpers (`provoke_record`, `provoke_wide_record`, `attach_ready`) are left alone.
- Proof runs on a tensorlake sandbox (`tl sbx`, the scode-ssh-delegate skill's sandbox path), created for the run and
  destroyed after. A local run is not evidence for this class; every one of these flakes passed locally.

## Design

Three additions to `crates/farhelm/tests/e2e/harness.rs`, one deletion pair in the modules that grew the originals.

### 1. `wait_for_replay_complete(rx, seen, secs) -> usize`

Drain the stream, appending `Data` to `seen` exactly as `wait_for` does, until the attachment's `ReplayComplete`
arrives; return `seen.len()` at that instant. Everything at or after that offset is live by construction — it left the
pty after the supervisor finished replaying history for this attach — so a test asserts on `&seen[live_from..]` and the
snapshot cannot reach it. Two contract points the docstring must carry:

- It has to be the FIRST wait on a fresh attachment. `wait_for` and `wait_for_after` swallow the marker (their
  `ReplayComplete => {}` arm), so a call after one of them never sees it and times out. The timeout panic names this
  rule, so the failure mode is a message and not a mystery.
- It bounds the attach's own catch-up and nothing later: a forced tmux pause replays history into an already-live
  attachment with no marker (supervisor's `service/connection.rs`, `send_replay_complete`'s docs). The three
  `a_forced_tmux_pause_*` tests in terminal_backpressure.rs find that replay by its `ESC c` reset and are not customers
  of this helper.

The supervisor emits the marker unconditionally between the initial replay and the live pump, including for a pane with
no history, so the helper never waits on output that will not come.

### 2. `normalize_pane_text(bytes) -> String`

Lossy UTF-8, then two transformations: strip every ECMA-48 escape sequence whole (CSI and two-byte escapes, the exact
grammar session_lifecycle.rs's `strip_escape_sequences` implements today, moved here with its docstring and both of its
unit tests), and trim trailing blanks from every line (the row padding a snapshot adds; a `\r` before the newline is
trimmed with them). It does not touch invalid bytes, because they are already gone by the time text exists; that is what
the live boundary is for. It does not parse string-type sequences (OSC), for the reason the current docstring gives.

`wait_for_normalized(rx, seen, needle, secs)` is the opt-in wait built on it: same loop as `wait_for`, the needle
matched against `normalize_pane_text(seen)`. Opt-in per call is the point. A read of every e2e module on 2026-09-03
found tests that deliberately look for escape sequences (`reattach_replays_history_and_modes` and the terminal_tabs
conformance test wait for `ESC[?2004h`; `reattach_to_alt_screen_app_preserves_content` and two forced-pause tests assert
the ORDER of `ESC[?1049h` against content), so the default `wait_for` keeps matching raw text and nothing changes for
the ~260 existing call sites.

To keep one loop instead of five, refactor `wait_for`, `wait_for_after`, and `wait_for_normalized` onto a private
`wait_until(rx, seen, secs, what: &str, pred: impl FnMut(&[u8]) -> bool)` core that owns the drain, the detach handling,
and the two panic messages. wrapper_launch.rs's `wait_for_settled_argv` is a fourth copy of that loop with a different
predicate and becomes a one-line call on the core. `terminal_backpressure::wait_for_bytes` is NOT folded in: it exists
to avoid the whole-buffer rescan on multi-megabyte transcripts, and the core rescans on every chunk exactly as
`wait_for` does today. Performance for existing callers is unchanged, not improved.

### 3. One `argv_marker` instead of two

hook_identity.rs and wrapper_launch.rs carry byte-identical `argv_marker`, `ARGV_MARKER`, `WIDE_COLS`, and `ROWS`. Move
the constants and the function to harness.rs as `pub(crate)`, implemented as: normalize, `rfind` the marker, take the
rest of that line, assert the width bound against `WIDE_COLS`. The trailing-blank trim the guard does by hand today is
then the normalizer's, and the docstring's account of the rc.1 gate failure moves with it. The width guard keeps its
shape and its reason: a row that really wrapped is full of argv characters to its last column, so trimming does not hide
it.

### What the audit found, and what is not converted

Every fixture in `crates/farhelm/src/fake_agent.rs` prints `FAKE-AGENT READY` at startup, and about 109 waits key on it
through a `TermStream`. Those waits are substring matches, which the snapshot shape satisfies as well as the live shape
does, so they are not converted; the race only bites a test that asserts on the SHAPE of startup output or on BYTES that
a snapshot may have canonicalized. The startup lines tests read beyond READY are `FAKE-AGENT ARGV:` (the argv guard,
converted above), `SELF-PID:`/`CHILD-PID:` (read with `marker_value`, which stops at whitespace and so is already
indifferent to padding), `POINTER:`/`INSTRUCTIONS:` in agent_relay.rs and `ENV:` in the env_echo consumers (substring
checks). The execution step re-runs this grep before declaring the audit closed:

```
rg -n 'wait_for(_after|_bytes)?\(' crates/farhelm/tests/e2e/*.rs | rg -v 'READY|echo:|RECORD-|HOOK-|CLONED|SPAWN|MOUSE-MODE|bye'
```

and reads each remaining site for an exact-line or byte comparison on a startup-printed row.

## Files touched

- `crates/farhelm/tests/e2e/harness.rs`: `wait_until` core; `wait_for` and `wait_for_after` on it; new
  `wait_for_replay_complete`, `normalize_pane_text`, `wait_for_normalized`, `argv_marker`; the constants; the two
  tokenizer unit tests plus one new unit test each for the boundary helper's first-wait contract (a stream that delivers
  data then the marker returns the offset; one whose marker was already consumed panics with the named rule) and for
  row-padding trim.
- `crates/farhelm/tests/e2e/session_lifecycle.rs`: `hex_tokens` calls the harness normalizer; `strip_escape_sequences`
  and its tests deleted; `non_utf8_terminal_output_survives_live_stream` asserts `0xff` on the live segment returned by
  `wait_for_replay_complete` instead of on the whole transcript (the on-request fixture stays; the boundary makes the
  claim exact rather than merely likely).
- `crates/farhelm/tests/e2e/hook_identity.rs` and `wrapper_launch.rs`: private `argv_marker` and constants deleted,
  `wait_for_settled_argv` reduced to the core call. Call sites unchanged otherwise.
- `crates/farhelm/tests/e2e/terminal_tabs.rs`: `terminal_conformance_holds_for_the_agent_and_for_a_tab`'s `0xff`
  membership check moves onto the live segment. Optional; the command that prints the byte is sent after the attach, so
  it is live already, and this is documentation by construction rather than a fix.
- `TODO.md`: the entry and this file are deleted in the PR that lands the work. `FLAKES.md` gets no new entry unless the
  proof run finds one.

## Proof

On a fresh 4-vCPU tensorlake sandbox with the pinned tmux (`scripts/build-pinned-tmux-ci.sh`), before and after the
change:

- the four converted tests plus the two argv-guard modules, 10 runs each in a loop, alongside a `cargo build` of the
  workspace as the load;
- the whole e2e binary three times at `--test-threads=4` (the release gate's shape), the ignored tests staying ignored.

The claim is that no attach-shape failure appears after the change (the "before" leg is expected to reproduce at least
one, as the 2026-09-02 runs did at 1 in 5 to 1 in 7). A failure in one of the four ignored tests, or on the time-budget
seam, is out of scope and gets a FLAKES.md entry rather than a fix here. Then the ordinary per-change battery from
AGENTS.md locally; the Playwright suite is untouched by this plan and needs no rerun for it.

## Effort: low

Rests on: the primitives are small and the escape stripper already exists with tests; the conversions are four tests and
one duplicated helper; nothing in product code moves, so no spec or protocol question opens. What could grow it: the
audit grep surfacing an exact-line comparison nobody remembers, or the "before" proof leg failing to reproduce, which
would make the "after" leg weak evidence and call for more repetitions. Half a day of code plus an hour or two of
sandbox time is the honest guess.
