# Narrow test runs

How to reproduce a test failure without paying for a whole battery. The full lists in AGENTS.md — the "Finishing work"
commands and the Playwright merge gate — are unchanged and still decide when work is done; this page is about the loop
before that. When you are chasing one failure, one flake, or one behavior change, start with the narrowest run that
could reproduce it and widen only when the narrow run will not.

NOTE: a flake first seen in a full-battery run is not evidence that only the full battery can reproduce it. Most of this
suite's historical flakes had local causes — a teardown race, a timeout tuned too tight, a poller losing to a slow
machine — and reproduce in a single-test loop. Concurrency-dependent flakes are real but rarer; the ladder below reaches
them at the later rungs. `docs/watch-items.md` carries the known flake fingerprints; check it before reproducing from
scratch.

## The ladder

Climb one rung at a time, and only when the rung below failed to reproduce:

1. The exact failing test, repeated in a loop.
2. Its module (Rust) or spec file (Playwright), still on one engine.
3. Its test binary at the release gate's thread count, or the spec file on both engines.
4. The full battery, exactly as AGENTS.md states it.

Repetition belongs on every rung: a flake that fires once in twenty runs needs twenty runs of evidence, and twenty runs
of one test cost far less than one run of everything.

## Rust: cargo test filters

The workspace holds roughly 2,100 Rust tests. Most are per-crate unit tests (`farhelm-helm` ~610, `farhelm-supervisor`
~590, `farhelm-ui` ~300, `farhelm-proto` ~100, `farhelm` ~100). The expensive battery is the one integration binary
`crates/farhelm/tests/e2e` (~330 tests driving a real supervisor and real tmux); `crates/farhelm/tests/` also has five
smaller process-level binaries (`agent_cli`, `spawn_cli`, and friends). Everything is selectable with cargo's ordinary
test filters — there is no bespoke runner.

Always put the pinned tmux first on PATH, exactly as the full suite does; the build is cached under `.ci-tmux/` after
the first call (about a minute of zig cross-compilation on 4 vCPUs), so this costs one subshell:

```sh
PATH="$(scripts/build-pinned-tmux-ci.sh):$PATH" cargo test ...
```

The shapes, narrowest first:

- One test, exactly:
  `cargo test -p farhelm --test e2e session_rename::a_rename_is_visible_in_the_next_list_reply_without_a_restart -- --exact --show-output`
  (the e2e binary's test names are `module::test_name`; get the real name from the failure output or `-- --list`).
- One filter (substring match): `cargo test -p farhelm-supervisor shutdown_acks -- --show-output`.
- A whole e2e module: `cargo test -p farhelm --test e2e terminal_tabs:: -- --show-output`.
- A crate's unit tests only: `cargo test -p farhelm-helm --lib provisioning:: -- --show-output`.
- Repetition for a flake: `for i in $(seq 20); do cargo test ... || break; done` — each invocation is a fresh test
  process, which matters for tmux-teardown flakes (`scripts/test-tmux-pinned-shutdown.sh` exists precisely because a
  surviving client or server can contaminate the next scenario inside one shared process; any of its ten single-test
  invocations can be run directly, copied verbatim from the script).
- Desktop-feature seams: `cargo test -p farhelm-ui --features desktop <filter>` — same mechanics, needs the
  webkit2gtk/gtk dev packages.

`--exact` needs the full path including the `tests` module for unit tests (e.g. `store::tests::the_name`); when in
doubt, use the substring filter or `-- --list` first. `--show-output` is worth keeping even on narrow runs — it is what
makes a loudly-skipped test (no systemd user manager, no passwordless ssh to localhost) visibly skipped rather than
silently green.

Two caveats before trusting a narrow non-reproduction:

- Thread count. The narrow run has no contention; the release gate runs its retained Rust targets at `--test-threads=4`.
  Ordinary CI does not run Rust tests, and the full e2e binary remains excluded from the release gate. Before concluding
  "only fails in the big run", reproduce that thread budget locally at rung 3:
  `cargo test -p farhelm --test e2e -- --test-threads=4 --show-output`.
- Ambient environment. A shell inside a farhelm session carries `FARHELM_AGENT_ID`, and the supervisor's sweep tests
  inspect live `/proc` environs. Four `service::sweep::tests` cases in `farhelm-supervisor` have failed under an ambient
  marker on machines with a systemd user manager (they skip themselves elsewhere). Scrub it
  (`env -u FARHELM_AGENT_ID cargo test ...`) or run from a shell outside any farhelm session before blaming your change.
  The separate `farhelm-teststate` sweep does not inspect process environments. Its earlier 2/10 versus 0/10 marker
  comparison did not establish causation: a later parallel crate run proved that its fixture's flock could remain held
  after the parent closed its descriptor, because another thread's child inherited it before exec. That fixture now
  explicitly unlocks before asserting reaping; `FLAKES.md` records the evidence.

The first cargo invocation in a fresh checkout still pays the dependency build whatever filter you pass; narrowing the
target (`-p`, `--test`, `--lib`) trims that too, since cargo only builds what the selected targets need. After that,
iteration is compile-one-crate plus the selected tests. For calibration, measured once on a fresh 4-vCPU sandbox: a
single supervisor unit test cost ~30s wall (compile of the just-built crate's test target; the test itself ran in
0.03s), a single e2e integration test ~40s wall (0.44s of test), and an 18-test e2e module rerun with everything warm
5.5s. Once the build is warm, the cost of a repetition loop is essentially the tests you selected and nothing else.

## Playwright: one spec, one engine

The browser suite is ~400 tests per engine across 33 spec files, and `e2e/playwright.config.ts` already defines one
project per (engine, spec file): `chromium-terminal-tabs`, `webkit-feed`, and so on. That makes file-level selection
first-class:

- Check what a selection matches (free, no stack boot): `npx playwright test --list --project=chromium-feed`.
- One spec file on one engine: `npx playwright test --project=chromium-feed`.
- One test: `npx playwright test --project=chromium-feed -g "a rename in one client"`. Measured on a 4-vCPU sandbox: a
  3-test spec file ran in 11s wall including the stack boot; one test at `--repeat-each=2` in 6.5s.
- Flake loop: add `--repeat-each=20`. Playwright restarts nothing between repeats, so this also probes state leakage
  between iterations of the same test.

Every run, however narrow, boots the real stack once (`start-stack.sh` via the config's `webServer`, a few seconds) and
needs the prerequisites built first: `cargo build` and the `dx build` web bundle, per AGENTS.md's merge-gate paragraph.
That cost is per-invocation, not per-test, so a `--repeat-each=20` loop pays it once. The stack resolves `tmux` through
PATH and the supervisor refuses a below-floor version, so on a machine whose distro tmux sits below the floor the same
prefix as the Rust suite applies: `PATH="$(scripts/build-pinned-tmux-ci.sh):$PATH" npx playwright test ...` — without
it, every test fails with a 409 "host 1 is connecting" from a supervisor that never came up.

Engine choice: chromium is the faster iteration engine; webkit is the desktop stand-in, so a report that smells
desktop-family (see `docs/desktop-web-triage.md`) should reproduce on `webkit-<spec>` before anyone blames shared code.
Selecting any spec in isolation is safe, including `terminal-multihost` — the config's ordering constraint (multihost
last) only matters for runs that include specs needing the second host after it, and a selected run inherits the
config's project order anyway.

## JS unit tests

`crates/farhelm-ui/js-tests` runs in milliseconds, so narrowing is rarely about cost — but selection works: from inside
the directory, `node --test client-log-shim.test.js` runs one file, and `node --test --test-name-pattern 'clipboard'`
selects by test name across files.

## The batteries without a narrow mode

- `scripts/test-install-sh.sh` — one monolithic pass over every install.sh scenario, no per-scenario selection. It is
  cheap enough (312 checks in ~7s on a 4-vCPU sandbox) that carving it up has not been worth it.
- `scripts/desktop-smoke.sh` — monolithic, ~5 minutes, and its legs share one built app and one Xvfb; there is no
  supported way to run a single leg.
- `scripts/test-provision-centos.sh` — already narrow where it counts: the cargo part is a single named test
  (`provisioning_and_update_over_ssh_preserve_an_operable_session`); the cost is the docker container and the one-time
  musl/tmux payload builds, which cache across runs.
