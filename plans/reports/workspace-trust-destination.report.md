# Workspace trust follows the destination

## What this was about

An agent launched with automatic trust in a managed checkout could spawn an inherited session in an ordinary folder and
carry that trust with it. The new folder's hooks, settings and commands could then run as if approved. Approval cards
showed the command flags without explaining trust, and replacing a recorded checkout with another folder at the same
path could still make it count as managed.

The maintainer chose destination-based trust for inherited spawns, a cheap plain-language card note, and an identity
check only if the existing comparison was enough. All three fixes fit those decisions. Each outcome has its own draft
PR, changelog fragment, completed ledger entry and feedback removal.

## Things you should know

Inherited spawns now resolve Codex, Muse and Pi trust once, before approval, from the destination's managed-checkout
membership and the agent's current Settings switch. Trust is removed in ordinary folders or when the switch is off, and
added in a managed checkout when it is on. Dispatch uses that saved choice. Once a card is shown, changing Settings does
not change the trust in that approved attempt.

Malformed stored launches are refused when their trust flags cannot be safely replaced. Removing only the trust label
would leave command flags behind; rebuilding the whole command would lose historical or custom words. The existing clone
normalizer preserves the other command words, and the original launch is checked before any rewrite. PR #2349 records
this refusal choice.

The optional retry key prevents a timeout retry from creating a second child session. An inherited spawn's key still
identifies its resolved launch, now including the destination-normalized trust. If Settings changes make that resolved
trust differ, a retry with the same key is rejected. It does not silently start a replacement with different trust. A
Settings change while an already displayed card waits is a different case: that attempt keeps the choice shown on the
card. The specification now states both boundaries.

Launch and Restart cards show the stored choice: “trusted by Settings in a managed checkout; repository hooks, settings
and commands can run without asking”, or “not trusted”. An unset choice adds no line; Claude remains outside this
change. The cheap-fix gate held because both card types already share one row builder and the launch already stores the
choice. No new protocol field, Settings lookup or production branch comparing harnesses was needed.

Managed-checkout membership now requires the folder at the recorded path to match its recorded identity. A missing or
replaced folder, a checkout record without captured identity, or an unconfirmed remount confers no trust. Remounting can
change the filesystem's device identity; a matching stable folder birth time can still confirm the same folder. The
small-change gate held: this reuses checkout recovery's existing identity comparison and releases the database lock
before checking the filesystem. It adds no identity algorithm, cache, lease or protocol. Unexpected filesystem errors
remain errors rather than being treated as ordinary membership.

This remains a point-in-time read. A later path replacement is still possible under the accepted out-of-band boundary.
The read does not retroactively rewrite managed-checkout marks already persisted on existing sessions; new directory
membership inspections use the stricter result. An unconfirmed remount can therefore lose automatic trust and the mark
returned by a fresh inspection, even when the same folder remains on disk.

## Open questions and possible follow-ups

None required for this plan. Native vendor-agent behavior and filesystem changes after inspection remain outside the
proof supplied by these focused tests.

## The PRs

- [#2349: bind inherited workspace trust to its destination](https://github.com/scode/farhelm/pull/2349/changes) — head
  `bf0303bdcadb275cdb0995f457e24fa0d83d7446`, based on main.
- [#2353: show workspace trust on approval cards](https://github.com/scode/farhelm/pull/2353/changes) — head
  `3fce547b210673ec1db56752cdc6201b8412d7c7`, based on #2349.
- [#2354: verify checkout identity before granting trust](https://github.com/scode/farhelm/pull/2354/changes) — head
  `d03e7cd5ff0e8b1f77febf8e9db2d0762f944ddd`, based on #2353.

## Checks run, reused and skipped

The stack was carefully rebased onto `9478d998f8414425e98e7857735aa74ce1fb4a96`. Intervening product changes refuse
blank running-agent prompts and clarify wrapper responsibility; they do not change trust normalization or directory
inspection. The remaining changes were bookkeeping and reports. Only feedback-index conflicts needed resolution,
preserving both plans' removals. Product, specification and test patches remained unchanged.

The restored runtime run uses final head `d03e7cd5ff0e8b1f77febf8e9db2d0762f944ddd`, with no uncommitted source edits,
fingerprint `1b7b8975cf95ffe742b4e1b2584929e26479e3305cc839b63849a1078dba2902`. It ran through the recorder in
development mode with nextest 0.9.148, pinned tmux 3.7c, four global slots and zero retries:

```sh
nextest_dir=$(python3 scripts/install-pinned-nextest.py) &&
tmux_dir=$(scripts/build-pinned-tmux-ci.sh) &&
PATH="$nextest_dir:$tmux_dir:$PATH" python3 scripts/record-test-run.py \
  --runner nextest --kind development --tmux required \
  --selection 'workspace trust inheritance, approval notes and directory identity' \
  --concurrency '4 nextest slots; retries 0' -- \
  cargo nextest run -p farhelm-helm -p farhelm-ui -p farhelm-supervisor --lib \
  -E 'test(inherited_spawn) | test(agent_clone_normalizes_trust) | test(agent_clone_keeps_the_trust) | test(agent_clone_checks_forged) | test(approvals::tests::) | test(managed_directory_snapshot) | test(directory_browse_reports_canonical_managed_membership) | test(verify_identity_reports_match_missing_and_different_object) | test(a_different_birth_time_is_a_different_object) | test(a_changed_device_number_with_matching_birth_time_is_the_same_folder)'
```

Recorder `f5b9dede-7c61-47be-b585-34833dd185f2` passed all 46 selected tests; 2,870 were selected out. The selection
also includes 20 neighboring helm approval tests because the shared approval-module filter matches both crates. It
covers twelve inherited-trust combinations inside one test, approval versus dispatch after a Settings change,
malformed-launch refusal and guard order, adjacent clone behavior, actual Launch/Restart card rows, production directory
inspection and existing identity and remount comparisons. Source and JUnit evidence are complete; child and recorder
exited successfully. Retained output is complete, with no runtime SKIPPED message or forced cleanup.

Source checks: `cargo fmt --all -- --check`, targeted
`dprint check SPEC.md SPEC_impl.md TRIAGE_OUTCOMES.md review_feedback_queue/INDEX.md`,
`python3 releasing/check-changelog.py format` and `python3 scripts/check-agents-md.py` passed on the rebased stack.
`cargo clippy --all-targets -- -D warnings` and the isolated interpreter's `python -B scripts/check-test-sleeps.py` also
passed on that source: 304 delays inspected, none missing a rationale.

An earlier helm run had six passes and one failure because the new Pi fixture requested Guarded, which Pi does not
offer. The fixture was corrected to its supported mode; the corrected test and the other focused selections passed
before interruption. Those old private records have disappeared, so they are historical observations, not retained
evidence reused for this delivery. The new run restores that evidence on the final source. No latent flake is claimed.

Broader Rust, browser, JavaScript, native desktop, installer and provisioning suites were skipped. This change adds
static text through existing card rendering and changes the trust composition and membership boundaries exercised above;
it adds no browser interaction, installation path or agent protocol. Live vendor CLIs were not exercised. No live
installation was changed.

## Review gate outcome

Each PR passed the required single fresh-context gpt-6.1-sol reviewer at high effort for correctness, design and
idiomatic code, against its full outcome, complexity gate and verbatim test-authoring checklist. No findings remained.
Commit and PR wording passed separate cold reads, and a documentation pass covered the touched files. These completed
review outcomes are recorded in the working log; their private artifacts disappeared during the interruption. The rebase
changed only mechanical feedback-index conflict resolution, so the reviewed source remains applicable. The independent
recovery check passed. This reconstructed report passed its fresh independent cold read.

### Landing

Landed as #2349, #2353 and #2354, merged bottom-up, unchanged from what the report describes. It was the first of eight
plans in this landing round. The next one, launch-enters-working-dir, failed its checks at landing and was blocked
rather than landed; the six after it were tested with this plan, without launch-enters-working-dir.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained link-hover-covers (link clicks refused under the Mac app's tunnel-error notice, and
the desktop file-opening fix) and plan bookkeeping; the base already included agent-spawned-session-rules, which also
edits the helm's agent requests. A fresh-context reviewer and I read this plan against those and against the seven other
plans in the round:

- The folder whose trust this plan settles on is the folder the launch then enters, including under
  launch-enters-working-dir's change, which the reviewer read against this plan before that plan was blocked.
- The approval-card note added here and link-hover-covers' changes to how refusal messages cover approval cards touch
  different parts of the card area; the approval and trust browser tests below ran on the combined result.
- Nothing else in the round touches workspace trust, inherited spawns or managed-checkout identity.

The only conflicts were in the review feedback queue's index, where every plan in the round removes its own entries; all
removals are kept. The landed code is identical to what was tested.

#### Fixes made during the landing

None.

#### Things the landing found

None of these needs action; they are recorded so the behavior is not a surprise later.

- Deciding whether a session's folder is a managed checkout now looks at the folder on disk, not only at its path. If
  that look fails for an unexpected reason (a parent folder the session's account cannot read, say), an agent's request
  to create a session with automatic trust is refused with that error, where before the path alone decided. The report
  mentions only that such errors "remain errors".
- An inherited spawn whose stored launch carries trust flags that do not match its recorded trust choice is refused,
  even on a host that otherwise allows YOLO launches without asking. Cloning a session already refuses the same case,
  and the report records the choice.
- The check that a command wrapper an agent supplies needs its own Allow or Deny card now also runs once earlier, for
  every agent create that can carry trust. Its result there is discarded and the card is still decided by the same check
  at approval, so nothing a user sees changes.

#### Checks

Run now, on this plan rebased onto main with the round's six other landing plans stacked above it
(launch-enters-working-dir removed after it failed):

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the isolated
  test-sleep check, `dprint check` on every changed file, `python3 releasing/check-changelog.py format` and the website
  build: all passed.
- Through the test recorder with pinned nextest and tmux, four slots and no retries: the whole protocol, helm,
  supervisor and UI libraries, the `farhelm` binary's own tests, the agent command-line test file, and the end-to-end
  tests for launches, terminal tabs, managed checkouts, structured launches, session lifecycle, agent relay, agent
  listing and wrapper launches: 3489 passed, run `cdabc40d-ecaa-418e-938f-01f2cbe4da64`. Its output has no runtime
  `SKIPPED` marker, so the end-to-end tests ran on the real systemd and tmux setup. The executor had run selected tests
  only.
- The UI's desktop-only Rust tests: 706 passed, run `4926be34-2adf-40e5-a588-663695e00161`. `node --test` in the UI's
  JavaScript test directory: 311 passed, run `fcb2b9cf-4424-4fc2-8148-dede8454a5ea`.
- After `cargo build` and the release web build, `npx playwright test` on header, terminal replay rename, session
  confirmations, approval layout, agent relay, workspace trust, Claude trust, quick launch, launcher template shortcuts,
  managed checkouts and tooltip coverage, on Chromium and WebKit, one worker, no retries: 242 passed, run
  `1484ba9a-0aab-4e7f-a7a4-716faede7c66`. These include the approval card and trust tests this plan's card note affects.
- Skipped: installer, provisioning, the native Mac app and the rest of the browser suite; this plan changes the helm's
  launch resolution, the supervisor's checkout identity check and one approval card line.

Nothing in the executor's report became untrue at landing.
