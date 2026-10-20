## What this was about

A helm that stopped serving during an install or update could release its state-directory lock while its blocking work
still used the release cache. Another helm could then claim the same directory. Earlier reproduction confirmed the
ownership gap but did not demonstrate corrupted installation bytes. The maintainer chose to close the gap within a
300-line size gate.

Both the CLI helm and the desktop's embedded helm now keep ownership until their runtime has finished shutting down. The
runtime waits for started blocking work before ownership can pass to a successor, including when serving panics.

## Things you should know

The token-control socket still closes when serving ends. During the remaining drain, `farhelm helm token rotate` can
fail after its deadline; `token show` retains its database fallback. This was accepted during planning. Ordinary desktop
Quit still exits the process as before.

The change is 268 lines of code, tests and specification diff, excluding the ledger, feedback removal and changelog
fragment. The fragment uses `kind: none`, as permitted by the plan, because the visible effect is limited to refusing a
second helm during this exceptional drain. The fix does not make the release cache support concurrent writers.

The regression runs a real helm with temporary state, parks blocking work on its runtime, observes serving teardown,
checks that another claim is refused, and verifies ownership becomes available after the worker and runtime finish.
Panic handling is established by the same declaration and destruction order; the runtime regression triggers embedded
shutdown, not a panic.

## Open questions and possible follow-ups

None required for this plan.

## The PRs

- [Draft #2383](https://github.com/scode/farhelm/pull/2383/changes): retain helm ownership through runtime shutdown.

## Checks run, reused and skipped

Run during execution:

- `cargo nextest run -p farhelm-helm --lib -E 'test(=tests::helm_ownership_outlives_runtime_drain) | test(=tests::a_second_helm_refuses_before_migrating_an_owned_state_dir) | test(token_control::tests::)'`
  through the recorder with pinned nextest 0.9.148 and tmux 3.7c, four slots and no retries: 21 passed, 1189 selected
  out, run `c49c4cc7-6a11-4091-8bf8-fee8a1fc61dc`, base `8d2dd2b65722a25c66e4c7fcbb2ea9e03965845b` plus the working
  diff, source fingerprint `e679a95783cf91477b12f50d7bfb16bd5d764d2ed31aba5dff805b46f0dd49fe`.
- A private negative control deliberately released ownership before runtime teardown. The exact new regression failed at
  its successor-exclusion assertion, run `d4a48c57-2b6a-4c5f-aa42-58de6e2f738a`. This was a controlled mutation, not a
  run of the unmodified base. The failure evidence was retained and the reviewed source restored byte for byte.
- After retaining the temporary state directory through fixture teardown, the exact final regression passed separately:
  `cargo nextest run -p farhelm-helm --lib -E 'test(=tests::helm_ownership_outlives_runtime_drain)'`, same recorder
  policy, 1 passed and 1209 selected out, run `594c4cd7-7576-4946-b0bd-b1eb0058bd32`, source fingerprint
  `f0107d21946678874964b2860a74391459a2c691aa8c561b02a23881d1963702`. Both successful runs had complete source, output
  and JUnit evidence, no forced cleanup, and no runtime `SKIPPED` messages.
- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, and `cargo check -p farhelm-ui --features desktop`: passed.
- The isolated `python -B scripts/check-test-sleeps.py` passed: 305 delays inspected, none missing a reason. Targeted
  `dprint check`, `python3 scripts/check-agents-md.py`, and `python3 releasing/check-changelog.py format` also passed.

These successful commands still cover the final change after its careful rebase onto
`2c71063f8eff5bfb3cfffa95b37ebb129ca9f925`: the full intervening diff adds a separate supervisor checkout-journal guard,
future-triage instructions and plan bookkeeping. None changes the helm runtime, ownership or either caller. The code and
specification bytes are unchanged. The feedback-index conflict kept both independent item removals; the later ledger URL
update changes no executable behavior. Markdown checks were rerun after rebase and that update.

Skipped the full workspace battery, browser, JavaScript, desktop runtime smoke and provisioning tests: the real-helm
regression covers the changed ownership boundary, existing tests cover token-control behavior, and compilation checks
both callers. No browser, asset or provisioning workflow changed. No external executor's evidence was reused.

## The review gate's outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort found no issues. It checked the runtime and lock
lifetime, both callers, the accepted token-rotation caveat, regression cleanup, bookkeeping and the 268-line size. The
commit title and body passed a separate fresh cold read. The final documentation pass found the changed lifecycle and
fixture contracts documented.
