# Typed launcher choices

## What this was about

The session launcher let people type `yolo` to select YOLO permissions, but selecting Guarded required `perms:guarded`.
The launcher also lacked search words for Goose and OMP approval modes and for the default reasoning effort. The
accepted typing audit kept the Agent / Command tab and its controls as documented exceptions, and kept `own` and
`custom` behind `perms:` because those words are common in ordinary searches.

The two draft PRs add bare `guarded`, `perms:approve`, `perms:smart approve`, `perms:chat`, and `effort:default` where
the chosen agent offers them. The existing scoped permission words remain. The specs and Start a session guide describe
the routes and exceptions; the TODO entry is removed.

## Things you should know

A short unlabelled query that is part of `guarded`, such as `ar`, can now show Guarded in the combined results. This
follows the accepted rule for bare `yolo`.

Review found that selecting an already selected default effort through search initially cleared a launch retry's
identity. That could create a second session after a successful server response was lost. The final change preserves the
identity when effort is unchanged; a browser regression confirms one retry key and one session. Review also found and
resolved a teardown race in that regression.

The changed documentation page is Start a session. This unattended, incidental docs edit did not start a preview server.

## Open questions and possible follow-ups

None identified.

## PRs

- [#2297: typed permission choices](https://github.com/scode/farhelm/pull/2297/changes) — draft, head `12bd94cb`.
- [#2301: default effort search](https://github.com/scode/farhelm/pull/2301/changes) — draft above #2297, head
  `960904ae`.

## Checks run

- PR #2297: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo check -p farhelm-ui --features desktop`, `dprint check`, `python3 releasing/check-changelog.py format`, the
  isolated test-sleep checker, website build with internal links, web UI build, and helm build passed. UI recorder run
  `5efdb8db-8fac-469e-ad0f-8605320db382` used
  `cargo nextest run -p farhelm-ui --lib -E 'test(launch_composer) | test(list::create_form)'` and passed 91 tests; run
  `e9989b7e-baaa-4861-8129-300a3a5adbbc` selected `bare_yolo_and_guarded_offer_permissions_and_preselect_exactly` and
  passed it after its assertion changed. Browser recorder run `c2991b9c-3aac-49a4-b853-4efe81ef1fb8` selected
  `composer-word-search`, `guarded-permissions`, `custom-permissions`, `goose-pi-composer`, `omp-composer`, and
  `tooltip-coverage` specs: 42 passes across Chromium and WebKit. The Rust runs used pinned tmux 3.7c and nextest
  0.9.148; no required substrate was skipped.
- PR #2301: the same source checks, desktop compile, website build and web UI build passed. UI recorder run
  `b4f2dba0-ba25-4807-9ba8-f65ad0b47079` used
  `cargo nextest run -p farhelm-ui --lib -E 'test(launch_composer::) | test(list::create_form::)'` and passed 92 tests.
  Browser recorder run `9c3ec956-5c13-47bb-9527-a71c111d2a02` selected the same six specs as PR #2297: 42 passes across
  Chromium and WebKit. After the retry-identity correction, browser recorder runs `3c261334-a3d0-4a91-b84c-8357297e9be8`
  and `315a03d4-15ca-49e7-86c8-0c5410391937` used
  `playwright test terminal-create-idempotency.spec.ts -g 'searching for unchanged default effort'`; each passed that
  one test on both engines. The last run covered the final test teardown. All recorder reports were complete, with no
  failed, skipped or retried selected cases. Clippy, desktop compile, cargo format and a web UI rebuild passed after the
  product-code correction. Dprint and the test-sleep checker passed after the final test edit; the checker inspected 304
  delays and found none without a reason.

## Checks reused

- PR #2297's broad UI and browser runs covered a working tree based on `96b8f46d` (source fingerprints `e8c35c261590`
  and `54e1faf27b09`); the narrow UI run covered `54e1faf27b09`. They preceded a clean rebase over two mainline UI
  changes. Those diffs changed host-picker blur behavior and choice layout spacing, without changing launcher search
  matching, permission availability or selection. The recorded behavior remained covered at PR head `12bd94cb`.
- PR #2301's 92 UI tests and 42-case browser run covered a working tree based on `12bd94cb` (source fingerprint
  `4d9b04562bc0`). They preceded the narrow retry-identity fix. They still cover the unchanged search and launch paths;
  the later two-engine regression covered the corrected no-op effort path (fingerprint `b46e899ec10e`) and final test
  teardown (fingerprint `3702535e19b6`). The helm build from PR #2297 was reused because PR #2301 changed no helm or
  supervisor code. The web UI built after the product-code correction was reused for the final run because the only
  intervening edit changed test teardown. Main moved afterward only for other plans' queue state, which does not
  interact with this stack.

## Checks skipped

- The full Rust and browser batteries were not run: the changed launcher behavior and the lost-reply case had focused
  coverage in both browser engines, and the plan changed no helm, supervisor, database or protocol behavior.
- CI was not dispatched. Its workflow runs only on demand, and its browser job is disabled; the recorded local browser
  runs supply the browser evidence here.

## Review gate

Each PR received the plan-required single fresh-context gpt-6.1-sol high review. PR #2297 had no findings. PR #2301's
identity finding and test-teardown finding were fixed and verified by follow-up review, with no remaining findings. Both
commit and PR messages passed a separate cold read.
