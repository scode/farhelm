## Since the last review

PR #3210 remains the only PR. Landing found three tests that expected an ordinary Stop annotation without proving the
agent had started. The recorded decision kept the product fix and required those tests to wait for the agent's own
readiness. Those corrections and the audit of similar tests were already reviewed and pushed. This round completes the
authorized cloud comparison; no further product change was needed.

## What this was about

Stopping an agent during startup could show a launch failure blaming the host's systemd user manager. It now shows
“Exited — stopped before the agent finished starting.” An ordinary Stop after startup still says “stopped by user.”

A recorded Stop takes precedence over failures inferred from a stopped terminal, including unfinished checkout setup.
This holds during normal observation, supervisor recovery and a retried Stop. A direct failure report from the program
that starts the agent still wins while Stop is pending; a completed stopped exit keeps its first annotation. Restart and
Replace share the corrected path.

## Things you should know

Stop can be recorded while Farhelm is reading launch evidence. The supervisor therefore checks the current stop intent
again when saving an inferred failure, and preserves whether a temporarily cached failure came from direct evidence or
inference. This closes the smaller race left by the initial outline without adding a session state or database schema,
or changing Stop's sequence.

Older pages show the new wording but use the ordinary exited icon in compact mode. Current pages use the stopped icon
for both annotations. The badge's length cap can shorten the text; the detail line wraps. These choices were accepted in
the plan. If Farhelm cannot reliably determine that startup was still incomplete, it keeps the ordinary Stop wording.

The returned tests now wait for the command's own ready output before expecting an ordinary Stop annotation. The tests
deliberately holding startup still stop before execution and assert the new wording. Neither behavior was weakened to
make tests pass.

## Open questions and possible follow-ups

No decision remains open. The previously blocked comparison is complete. A focused corrected run does not establish that
the whole end-to-end suite is free of other failures.

## The PRs

- [#3210 — report stops during agent startup](https://github.com/scode/farhelm/pull/3210/changes), one draft based on
  main, head `09b1c766`.

## Checks run, reused and skipped

- This round built matching native and release web artifacts, then matched nextest archives on one base. The baseline
  reverses only the three readiness corrections. Both archives carry the identical production binary, fixture binary and
  pinned tools; their source and archive identities are retained privately. The fresh image's standard browser probe
  passed 20 welcome cases on Chromium and WebKit, recorder `8ab5c872-6ece-4874-bcca-199a31f9583b`.
- The unfixed cloud selection ran the three reported cases with `cargo nextest run --archive-file <archive>` and
  `--stress-count 20 --no-fail-fast`, recorder `88112f8a-9623-4af4-a0b3-b30782bf43c8`. All 20 iterations completed: one
  passed and 19 failed, comprising 54 failed and 6 passed case executions. Each of the three tests reproduced the exact
  mismatch: expecting “stopped by user” while observing “stopped before the agent finished starting.” The first
  iteration passed, so this is a timing-dependent failure. Complete failure output, source identity and traces were
  retained before the corrected run; the recorder ended normally with the test-failure exit 100.
- The corrected cloud selection ran both restart modules and both startup Stop controls once:
  `cargo nextest run --archive-file <archive> -E '<restart modules and startup Stop controls>'`, recorder
  `9b9cbf01-3311-4f9a-a0c7-f752d666313d`. All 28 selected tests passed in 10.744 seconds; 518 were selected out. Each
  formerly failing test passed. The deliberate unscoped and real-scoped startup controls actually ran, taking 5.251 and
  5.326 seconds, and retained their early-Stop assertions. Complete console and summary output is retained.
- The cloud archive runs use the standard runner's generic recorder mode, pinned nextest 0.9.148 and tmux 3.7c, four
  test slots and zero retries, on requested four-CPU/eight-GB Tensorlake workers. Both summaries record systemd as PID
  1, successful lingering, a running user manager, the offline Podman image and working self-SSH. Neither output
  contains a runtime `SKIPPED` marker. Exactly five owned sandboxes were created, at most one running at a time. Every
  owned sandbox and snapshot, including the fresh template, was deleted and verified absent afterwards.
- Reused the previous corrected local selection, recorder `b4722603-cc87-48e3-a216-1d7578e4ea39`:
  `cargo nextest run -p farhelm --test e2e`, filtered to both restart modules and the scoped/unscoped startup Stop
  controls. All 28 actual cases and one separate fixture setup passed, with complete output and no runtime skip. The
  tested source fingerprint was `d7ff0c7b8d2bb6152098f425b09f862e1b4bea834fec4c906ec5065eaf390942`.
- Reused the original product gates below on unchanged code. Their earlier delivered report and working log preserve the
  results and source identities. The original raw records are no longer present on this host, so their complete argv and
  exact filter strings cannot be independently recovered here. The commands below identify the recorded selections, with
  unavailable selectors shown as placeholders; they are not reconstructed exact argv.
- Primary 10-case regression: `cargo nextest run -p farhelm-supervisor --lib -E '<primary Stop regression selection>'`,
  run `ef8f6306-4c81-43cb-a230-a76cb41416bc`, covered `11b62255`. It checked pending Stop, both recovery branches,
  retried Stop, direct versus inferred evidence, cached status, spec-read fallback and first-annotation retention.
- Surrounding 37-case regression:
  `cargo nextest run -p farhelm-supervisor --lib -E '<launch evidence and cleanup selection>'`, run
  `d3b47bde-4898-4ac3-8a12-5bf6e2327fc6`, covered `7a86f472`. It checked genuine launch failures, cached errors when
  writes are unavailable, artifact cleanup, ordinary Stop and terminal-host exits.
- Compact-row rendering:
  `cargo nextest run -p farhelm-ui -E '<compact_rows_draw_both_stop_annotations_with_the_stopped_icon>'`, run
  `8ce73bc5-be8a-4a4f-8679-1d29c2d037a2`, covered base `b948a30b` with the UI/protocol working patch, source fingerprint
  `f744088ad8303d7ac85ef0314261c641f2f474e20b6d49bc83ee34c92fc69005`. No exact working-copy revision was recorded. The
  same UI/protocol edits were subsequently integrated unchanged; the test checked both stopped icons and the full
  accessible and hover wording.
- Four real lifecycle cases:
  `cargo nextest run -p farhelm --test e2e -E '<startup Stop and genuine launch-failure selection>'`, run
  `e75cf4ab-dc7f-4f27-bcb8-9dd8389d1205`, covered `7a86f472`. The scoped and unscoped startup Stop controls and genuine
  wrapper/login-shell failures passed, with no runtime substrate skip. Its fixture setup was separate.
- Reused `cargo clippy -p farhelm-supervisor -p farhelm-proto -p farhelm-ui --all-targets -j 4 -- -D warnings` on
  `7a86f472`, the corrected supervisor test-target Clippy on `11b62255`, and the returned-test
  `cargo clippy -p farhelm --test e2e -- -D warnings` on the source later pushed as `c8f49d54`. The later readiness
  changes affect neither the product code nor the earlier rendering and classifier coverage. Matching formatting,
  fragment and isolated test-delay checks also passed; the latter inspected 423 delays with no missing rationale.
- The earlier 39 unfixed local case passes did not establish this comparison. The original fixture failures
  `91efa7d0-f3b7-4a55-b033-3030f6c14bf9` and compile refusal `1aa3c166-3382-4158-9fb9-796816e44318` remain failed
  observations. Corrected fixture runs passed afterwards; no failed command is counted as a pass.
- This round passed `cargo fmt --all -- --check`, changed-file `dprint check` and changelog-fragment format. The careful
  rebase onto `6635456c` preserved the feature patch exactly. The intervening Mac provisioning, Rust 1.99.1 pin, effort
  label and plan-monitor changes leave the Stop classifier and readiness helpers unchanged. Rust 1.99.1 also built the
  comparison archives. The subsequent changes through `137eeaff` add future plans and queue decisions, plus an
  executable-bit change to the unchanged cloud runner; none changes runtime behavior or the comparison's inputs. The
  feedback-only commit `5fa13111` was audited in full and does not touch the plan, so the PR stays based on `137eeaff`
  without another rebase or runtime run.
- Skipped the full workspace and browser batteries, native desktop, installer, provisioning and hosted CI. The changed
  behavior has deterministic classifier, rendered-row and focused lifecycle coverage; the browser edit is a comment.
  Remote hosts, terminal hosts, Restart and Replace share the corrected supervisor paths, as the plan allowed.

## Review gate's outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort found no correctness, design or idiomatic-code
issues in the full feature, including the three returned readiness corrections and the audit of similar Stop tests. The
separate scope assessment supported the commit-time distinction between direct and inferred evidence. Commit and PR
wording passed its independent cold read. Every changed file received the documentation pass. The rebases preserve the
reviewed feature patch, so that code review remains applicable.
