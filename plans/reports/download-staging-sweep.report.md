# Desktop download cleanup on Quit

## What this was about

Quitting the desktop app during a download could leave hidden partial files in Downloads because normal exit skips the
save object's cleanup. Triage approved a fix within about 100 lines. The first prototype recovered leftovers at startup
using ownership locks, but it could not safely classify older unmarked directories and was blocked without publication.

The maintainer then chose cleanup on normal Quit instead: track this process's active staging folders, remove them
during event-loop teardown, and refuse new saves once quitting. That decision explicitly leaves earlier-build, crash and
force-quit leftovers alone. The draft implements that replacement design in 99 lines of code, tests and implementation
notes, within the gate.

## Things you should know

Cleanup runs before the desktop event loop's hard exit and does not wait for transfers to finish. It removes only
directories registered by this process; it scans no directory and uses no inter-process locks or ownership markers.
Completed saves outside staging survive. A completion already publishing as Quit begins may still leave its completed
file, consistent with the existing publication race. Ordinary failed or cancelled transfers retain their cleanup.

Pinned Dioxus and Tao source confirms that the hook runs before process exit, including the macOS
application-termination callback. This is source evidence plus a desktop compile check, not execution of macOS Quit. The
regression keeps actual partial bytes and their save owner alive while cleanup runs, then checks removal, preservation
of a completed save and a separate owner's staging, and refusal before a new staging directory can be created.

The feedback item was removed, and its triage record now reflects the maintainer's revised criteria and this PR.

## Open questions and possible follow-ups

No decision remains open. Earlier-build, crash and force-quit leftovers remain intentionally uncovered. The normal macOS
Quit path was not exercised on this Linux executor; a future native test could verify that integration directly.

## The PRs

- [#2440: clean partial desktop downloads on quit](https://github.com/scode/farhelm/pull/2440/changes), draft against
  main.

## Checks run, reused and skipped

- Recorded `cargo nextest run -p farhelm-helm --lib -E 'test(downloads::)'`: 17 passed, 1216 selected out, zero failures
  or retries. Run `a5d4f6bf-6c64-488f-a8bb-4c0a3344b57f`, nextest 0.9.148, pinned tmux 3.7c. Source identity and JUnit
  are complete; output reached EOF without truncation and process cleanup completed. No runtime substrate-skip marker
  was present. Coverage includes the new Quit regression and existing save, cancellation, publication and opening tests.
- `cargo clippy --all-targets -- -D warnings` and `cargo check -p farhelm-ui --features desktop` passed.
- Cargo formatting, changed Markdown dprint, changelog-fragment format, instruction-pointer checker and test-delay
  checker passed; the latter inspected 305 delays with no unexplained delay.
- These commands covered base `ab2d2e02` plus the implementation. Their evidence is reused after a careful rebase onto
  `18a544c1`: the full upstream change contains only unrelated review records and their index. The functional diff is
  unchanged. Markdown and instruction checks were rerun for the combined tree; the final ledger PR link cannot affect
  executable coverage.
- The old startup-sweep prototype's results were not reused for this replacement. No full Rust battery, browser,
  website, installer or hosted CI run was warranted: this changes native staging ownership and an exit hook, covered by
  the focused Rust cases and desktop compilation. No actual AppKit runtime result is claimed.

## Review gate outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort reported no findings after checking correctness,
design, language idioms, the revised acceptance criteria, the full test-authoring checklist and the 99-line size gate.
The mandatory independent resume check passed, and the commit message passed its fresh cold read. A separate
documentation pass covered every touched code file. No substantial departure from the revised design or repeated
corrective review required another scope checkpoint.

### Landing

Landed as #2440, with the same content the report describes, rebased onto this round's main. It was the first of six
plans in this landing round, which stacked every plan in queue order and checked them together before any merged.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained round 108's sixteen plans, a TODO entry, and plan bookkeeping. A fresh-context
reviewer and I read them and the other five plans of this round against this plan.

- transfer-leftover-cleanup, from round 108, changed how the helm forgets a cancelled download or upload between the
  helm and the host. This plan changes the desktop app's save into Downloads, in different code; a save still running
  when Quit cleans up fails to publish because its staging folder is gone, and its transfer bookkeeping is released as
  before.
- The cleanup does not touch the helm's shutdown or its state-folder lock, and an ordinary Quit is still an immediate
  exit, as SPEC.md's "Desktop Quit" requires.

No other plan in the round touches the desktop app's quit path. For context: SPEC_impl.md's "Cleanup on quit" section,
already on main when this plan ran, is the rule its revised design follows, and the review confirmed the diff matches
the plan's recorded decision: clean up this process's own staging folders when the app quits, refuse new saves once
quitting, and add no start-up sweep and no locks or ownership markers. The only conflicts were in the review feedback
queue's index, where each plan removes its own entries; all removals are kept.

#### Fixes made during the landing

None.

#### Things to know

- The cleanup runs on the app's main thread and can briefly wait while a save is creating its folders in Downloads. The
  wait is short and bounded; no follow-up is needed.
- As the report says, the normal macOS Quit path was checked from the pinned framework source and a desktop compile, not
  by quitting a real Mac app.

#### Checks

Every plan in this round was executed while round 108's sixteen plans were still unlanded, so none had been tested
against them. Together the six change a menu focus check that every menu uses, which session a page opens on load, the
desktop app's quit path, helm preferences and storage, and a browser-test helper many specs share. That is why this
round ran the whole Rust workspace and the full browser suite. All of the following ran on all six plans stacked
together on main, including the fixes made while landing them:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the isolated
  test-sleep check, `dprint check` on every changed Markdown, JSON and TOML file,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` and
  `scripts/check-desktop-assets.sh`: all passed (formatting and the test-sleep check again after the last test fixes).
- `cargo nextest run --workspace --exclude farhelm-desktop` through the test recorder with pinned nextest and tmux, four
  parallel test slots and no retries, run `8c534e03-3c81-4f66-99e3-0e6ffdb526e8` (kept): 3951 passed, 3 failed. Two were
  helm database tests fixed during this landing (see select-newest-on-close's notes); the helm's checkout configuration
  test module then passed, 28 tests, run `faf4d80d-ec6d-4a36-9220-9d522e8f38d0`. The third was
  `terminal_backpressure::shallow_pause_resumes_without_reset_or_replay`, a known open flake under load (FLAKES.md and
  the TODO deflake entry "Shallow pause resets under load"); no plan here changes terminal streaming or the supervisor.
  Five repeats of that test by itself, with the browser suite running at the same time, gave four passes and one failure
  (run `e34c4af0-729c-4f62-b1bb-db1c6d9e97e2`, kept).
- `cargo nextest run -p farhelm-ui --features desktop`, run `03589e5a-0f7e-423b-ab65-5c3bdd33c33c`: 720 passed; this and
  the desktop compile check above cover the desktop quit change, which lives in the UI crate's desktop code.
  `node --test` in `crates/farhelm-ui/js-tests`, run `c600b345-76c4-42a8-a879-fce21765c8d6`: 328 passed.
- The full browser suite (`npx playwright test` in `e2e/`) on Chromium and WebKit, one worker, no retries, in two parts.
  The first run, `68484e27-6c23-45a6-aae5-5539e8c275b2` (kept), was cut short part-way by my own mistake: a narrow
  re-run I started in the same checkout removed files the running suite depends on, so every test it started afterwards
  failed before testing anything. Its results stand for the 64 projects (one engine and one spec file each) whose tests
  all started before that point and passed. Every other project, and the spec files changed by later landing fixes, ran
  again on both engines in run `49ead42e-f608-4e76-a833-6ff945dfb1f8`: 1538 passed, 12 skipped by design (WebKit
  clipboard cases and tests that need a real signed-in agent or run on one engine only), and 5 failed. Those 5, and the
  4 the first run had shown before that point, were all tests that assumed how a page behaves on load (which session it
  opens, or how many preference reads and saves it makes) before select-newest-on-close changed it. They are 6 distinct
  tests, all fixed in that plan and passing in narrow runs on both engines (listed in its notes).
- Skipped: the workspace and desktop doctests (no plan adds or changes a documentation example),
  `cargo check -p farhelm-desktop` and the desktop smoke test (the desktop app's own crate is unchanged; its quit path
  change is in the UI crate covered above), the native Mac app (not runnable here) and the installer (unchanged).

The report's checks ran on a main from before round 108's landings, so they no longer describe what landed; the combined
runs above supersede them. Nothing else in the executor's report became untrue at landing.
