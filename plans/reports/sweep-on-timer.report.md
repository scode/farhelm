### Since the last review

The original timer-owned sweep work in #1758, #1761 and #1766 is already on main. This round adds one draft PR,
[#1792](https://github.com/scode/farhelm/pull/1792/changes), to address the maintainer's returned case: sessions whose
agents exited before a host reboot kept checking launch files on every tick. None of the original PRs was reopened or
changed.

The maintainer allowed a database change subject to a gate on the complexity of the resulting code, and explicitly left
same-boot missing-pane retries in place. This follow-up uses one saved boolean in the existing reboot and restart
transactions. The scope and source reviews found no significant extra complexity.

### What this was about

A stopped session normally stops checking its startup-error file and checkout-preparation state once those checks
complete and its launch can no longer write. But an agent that exited on its own before a host reboot retained its saved
exit, rather than becoming interrupted. After the reboot there was no terminal pane to establish that its launch was
finished. Its checks consequently repeated every two seconds, including after later supervisor restarts.

The supervisor now remembers that an observed host reboot ended every retained launch, including an already-exited one.
Successful checks that find no startup error or failed preparation can then stop repeating. The session retains its
saved exit code, annotation and restart behavior.

### Things you should know

The saved fact is that the old launch cannot write again. It does not mean its existing files are error-free: they must
still be checked successfully before repeated reads stop. A failed read, an error awaiting persistence, an inconclusive
file check, or a supervisor without permission to record outcomes remains retryable. Merely missing panes in the same
boot remain retryable, as requested.

The fact survives later supervisor restarts within the new boot. Renaming keeps it; a new launch clears it. If Restart
definitively fails before starting anything and successfully restores the prior run, it restores that run's saved fact
too, while keeping the existing safeguard against an old operation changing a newer launch. A potentially started launch
or failed restoration does not inherit it.

Schema 30 adds the false-default field. There is a historical limit: if an older supervisor already recorded a reboot
before upgrading, it left no fact distinguishing those old exited sessions from same-boot missing panes. Those unknown
rows continue retrying until a reboot observed by the new code establishes the fact. Inferring it during migration would
change the expressly preserved same-boot behavior. The migration does not reconstruct history.

There is no protocol change, per-tick database write, extra background task, new lock or change to list freshness. No
CPU percentage improvement is claimed; this round proves that repeated launch and preparation reads stop under the
intended conditions.

### Open questions and possible follow-ups

No decision blocks delivery. Retrospective settlement of rows whose reboot was already consumed by an older supervisor
would require another decision about indistinguishable historical rows. This round preserves the same-boot guarantee
instead of guessing.

The original report's pre-existing preparation-classification risk remains outside this follow-up: a transient empty
pane query while checkout preparation is still running can display Error. It was not reproduced here, and this round
does not claim to fix it.

### PRs

- [#1792](https://github.com/scode/farhelm/pull/1792/changes) — stop repeated launch checks after an observed host
  reboot. One new draft; pushed head `35f9e455edf62728b77c596a79af5cb964945b11`, based on
  `6394e31a599f67be6feaeaa07cc5fd9002749702`.

### Checks run, reused and skipped

Rust execution used the recorder, pinned nextest 0.9.143 and tmux 3.7c, four global slots and zero retries. Targeted
validation ran in an owned Linux resource scope limited to four CPUs, 24 GiB memory and 2,048 tasks.

The baseline exact reboot regression, run `9c9a0cfe-8c97-4aa2-bfa6-42fffbe1191b`, failed on unchanged product code at
the post-reboot settlement assertion after establishing the saved unannotated exit and absent panes. That is retained
regression evidence, not a passing run.

The focused command was
`cargo nextest run -p farhelm-supervisor --lib -E 'test(store::tests::) | test(settled_launch_reads) | test(unsettled_launch_reads) | test(boot_interrupted_launch_reads) | test(pre_reboot_exited_) | test(aborted_restart_republishes_) | test(a_relaunched_entry_) | test(a_renamed_entry_)'`,
through `scripts/record-test-run.py` with `--runner nextest --kind development --tmux required`. Run
`fbb364c5-bf19-4899-b66f-4389c0d59fb5` executed 121 selected cases: 120 passed and one failed at a new test's setup
assertion. This run is failed evidence; its 961 selected-out cases supply no coverage.

The passing cases cover both startup-error and preparation reads for pre-reboot exited sessions, later same-boot
supervisor restarts, authority/read-error retries, same-boot pane absence, schema migration/reopen, boot-transaction
rollback and refusal of stale restart updates. The failed test incorrectly assumed startup had not created the launch
directory. Exact reproduction `75db8a69-2204-4d75-81d6-6aef00359f21` confirmed that fixture error. The correction
replaces only an empty fixture directory with the pre-spawn obstruction; it refuses unexpected contents instead of
deleting them. Exact corrected run `5700adf3-2344-4279-939b-38f54fa33f0e` passed one of one and proves the aborted
restart restores both saved and published reboot knowledge. No selected runtime substrate skip appeared.

The 120 successful cases from the failed run are reused individually for source based on `6394e31a`: only the failed
test's setup/comment and a redundant Arc clone in the passing preparation test changed afterwards; the clone became a
borrowed one-entry slice with the same target and await lifetime, leaving product source, test inputs and oracles
unchanged. The corrected exact run closes that case's gap. All three failed records remain retained; no later pass
replaces them, and these are same-session regression/setup observations rather than latent flakes.

`cargo clippy -p farhelm-supervisor --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`
passed in bounded resource scopes, serially with cargo. The latter checks the shipped configuration with test seams
disabled. The first supervisor lint found a redundant Arc clone in the new preparation test; its equivalent
borrowed-slice correction passed the final lint and source-review follow-up. The initial failed lint log remains
retained.

`cargo fmt --all -- --check`, changed-document `dprint check`, `python3 releasing/check-changelog.py format`, and the
isolated `python -B scripts/check-test-sleeps.py` passed. The delay checker inspected 278 delays with zero missing
reasons. The initial document formatter refusal concerned paragraph wrapping in the implementation specification; the
corrected document passed.

Upstream executable and specification changes through `8bc1af9a` concern OMP conversation ownership in a separate
process-attribution path and specification paragraph. Queue/report and feedback-intake changes do not alter the boot
transaction or launch-read contract. The follow-up has no interaction requiring a behavioral rebase at that point.

Full Rust and browser batteries, JavaScript, desktop execution, installer, provisioning, website, doctests, release and
hosted CI were skipped: the current store/boot/read/restart proofs cover this bounded follow-up, which changes no UI,
protocol, launch grammar, executable example, installation or deployment behavior. The optional CPU comparison remains
skipped on the shared loaded machine. No live installation was changed.

### Review gate outcome

The required fresh native source reviewer was configured as gpt-6.1-sol at high effort. It read the full plan and latest
decisions, every changed file and the test-authoring checklist verbatim, and reported no findings. Its follow-up
accepted the fixture correction. The scope reassessment accepted the one-bit design, and the final source review found
no unnecessary resulting-code complexity. These are source-review conclusions; the executor supplied the runtime results
above.

Commit and PR wording passed a fresh native gpt-6.1-sol medium cold read after a rewrite removed unexplained launch-read
jargon. Resume reconciliation used the required fresh inherited-model native reader. The required separate fresh native
report cold read passed on the executing session's inherited model.

Implementation and investigation stayed local; only prescribed reviews and cold reads were delegated. Requested native
model configurations are exposed, but actual runtime model identity and usage counters are unavailable. Private review
evidence remains under session UUID `d0e361cc-2cf7-49dd-a656-7f0f6a284fbf` and in the plan's working log beside the
checkouts.

### Landing

Landed on 2026-10-10 (UTC) as #1792 (stop repeated launch checks after reboot), one squash commit on main, in the plan's
follow-up round, first of the three plans landed together in this round (then waiting-sound and terminal-file-download).
The rebase met one textual conflict, in SPEC_impl.md, where hook-report-watch had added a sentence on the periodic pass
skipping sessions busy with a report next to the text this round changes; both are kept.

#### Review before merging

A separate reviewer read the round by reading the code only. Case 1 is fixed: a launch's "its boot has ended" proof is
recorded for every retained session in the same database transaction that handles a reboot, survives later supervisor
restarts in the new boot, is cleared for every new launch, and is restored only when a failed restart is definitively
abandoned. Case 2 is unchanged: within one boot, a merely missing pane without that proof keeps being re-read. It found
no path that settles a launch that could still write, and judged the change (one database column, one statement in the
reboot transaction) proportionate to your complexity gate. Sessions whose reboot an older supervisor already handled
keep being re-read until the next reboot the new code sees, as the report says; that includes existing installations
right after upgrading.

#### Fixes made while landing

- The supervisor's test of upgrading a version-17 session database still expected the database to end at version 29;
  this round adds version 30, so the test failed in the combined run. The landing updated it to 30 and named the new
  step in its message.
- SPEC_impl.md now records that older supervisors refuse the version-30 database after a downgrade, as it does for the
  two previous versions, and two code comments the reviewer found garbled or backwards were rewritten.

#### Checks

All three plans of this round (sweep-on-timer's follow-up, waiting-sound, terminal-file-download) were stacked in
landing order and checked together.

- Run now: `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo check -p farhelm-ui --features desktop` and `cargo build` (clean after the landing's fixes below); the
  supervisor, helm, UI and protocol unit tests in full with the file-download end-to-end tests, through the recorder
  with pinned tmux 3.7c, four slots and no retries (run `6225cfec`, 2699 of 2700; the failure is sweep-on-timer's
  database upgrade test, fixed as described in its notes, after which the upgrade, schema and store tests passed 113 of
  113 in run `106eaf3c`); the UI JavaScript tests run directly (226 of 226); the desktop asset comparison (run
  `d09df54a`, 24 assets on both sides); and, on Chromium and WebKit with one worker and no retries, the sounds,
  settings, change-feed, sort, stale-read, filter, approval-layout and terminal-file specs (run `208a7db7`, 122 of 122),
  which include the four specs the first waiting-sound round broke.
- Between the checks and the merges, main gained the 2026-10-10 triage decisions written into SPEC.md (#1801) and its
  plan (#1802). They touch none of these plans' subjects; the only effect was a textual SPEC.md conflict in the last
  terminal-file-download PR, resolved by keeping both texts. No checks were re-run for that.

The report's checks predate the rebase; nothing else in it was made untrue by the landing.
