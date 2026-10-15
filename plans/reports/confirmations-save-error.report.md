### What this was about

Turning off Settings’ “skip session confirmations” switch restores the questions before Delete, Replace, Restart and
closing a terminal tab. Previously, if saving that choice failed, nothing warned the user; reloading could bring back
the older saved opt-out and make those actions happen on the first click again. The maintainer chose “show error”.

### Things you should know

Settings now says “Confirmations may still be off for this helm. Try again.” beside the switch when restoring
confirmations fails to save. The current page keeps asking for confirmation, as the user chose. A newer switch click
clears the warning, and an older save cannot attach its failure to that newer choice. Turning the opt-out on keeps its
existing silent failure behavior: losing that convenience is safe.

Writes keep their existing ordering and survive closing Settings. The specifications now distinguish the restoring
direction from the silent preference exceptions. The original finding and its index entry are removed, with the retained
outcome recording this execution. No automatic retry or switch lock was added.

### Open questions and possible follow-ups

None. This change reports failure; it does not promise that a failed save changed the helm’s stored choice.

### PRs

- [#2054](https://github.com/scode/farhelm/pull/2054/changes): show confirmation restoration save errors; one draft PR
  based on main. It has not been marked ready or merged by this executor.

### Checks run, reused and skipped

Run for this change:

- `cargo clippy -p farhelm-ui --features desktop --all-targets --offline -- -D warnings` passed. This checks the changed
  UI and its native configuration; unrelated workspace lint targets were not selected.
- The recorded UI preference-queue selection with desktop support passed all 17 selected tests, including failed-save
  recovery, acknowledgment cancellation, independent fields and returning to an in-flight value. Another 617 tests were
  filtered out. The selected tests had no runtime substrate skips. Run: `342a41e5-bbc3-47fc-bd16-31513c0ba5c2`.
- The CLI/fixture and release web builds passed against the combined tree.
- The recorded confirmation and Settings browser specs passed all 26 cases on Chromium and WebKit, with one worker, zero
  retries and pinned tmux. There were no skips, failures or incomplete cases, and stack cleanup completed. Run:
  `b1d33cc5-b9ec-430f-baa5-2b16611efae4`. The selected cases use the local host; the optional second SSH host was
  unavailable, so this makes no remote-host execution claim.
- `cargo fmt --all -- --check`, applicable changed-file `dprint check`, the changelog format lint and the isolated
  source-only sleep checker passed; the checker inspected 299 delays with none missing a rationale. The diff whitespace
  check passed. Dprint has no TypeScript plugin here, so it does not check the browser file.

The rebase onto `688aeee3` brought in terminal file opening, documentation decisions and queue bookkeeping. The complete
incoming diff was inspected. None changed the preference queue, its callers, this Settings control or the error policy
being narrowed. The fix’s Clippy evidence remains applicable; the rebased desktop UI test target compiled the incoming
native changes, and the web build uses the combined tree. The final rebase onto `e2d89db2` added conversation-tracking
policy and bookkeeping only. The code and tests stayed byte-identical; the feedback-index conflict preserved both sides’
removals. Runtime results and source review were reused, with Markdown formatting and whitespace checked again.

The full Rust workspace, unrelated browser specs, native app runtime, installers and releases were not run: they do not
cover a remaining risk in this preference-save change. Chromium and WebKit cover the browser UI; they do not prove the
desktop shell’s native lifecycle. No maintainer installation was used.

### Review gate’s outcome

The required fresh-context external `gpt-6.1-sol` high review found one test-fixture defect: simulated failure replies
omitted the helm’s build stamp and would trigger unrelated version-mismatch behavior. Both replies now use the existing
stamped-response helper. A follow-up review confirmed that correction and the strengthened off/on/off test, which keeps
evidence of even a transient stale warning. It reported no remaining correctness, design or idiomatic-code findings.
These were static reviews; the runtime evidence is listed above. A separate cold read accepted the commit and PR
wording. A documentation pass covered the touched code and tests.

Implementation and investigation stayed in the executor as required. Source review used external sol high; wording
review used native sol medium. Private evidence is in galaxy-brain session `f3bba8d1-cfeb-4c58-a67b-11f4d7f260c3`.
External per-turn counters are retained; exact reported model attribution, native reader usage and executor usage were
unavailable. No usage totals were inferred.
