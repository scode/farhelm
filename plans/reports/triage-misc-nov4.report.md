## What this was about

Four triage outcomes covered an incomplete template approval card, host-controlled text in debug logs, a terminal
recovery limit and an old-template finding. You chose to show the whole resulting template, escape all peer-supplied
text the helm logs or retains, name the accepted shell-paste consequence after a recovery gap, and discard the finding
about wrappers saved by older releases.

## Things you should know

Template approval cards now show the container image, proxy port and GitHub token-file path, or an explicit "none" when
the write removes the container setting. The card shows the path without reading the credential. This repairs the
template card only; the launch card is outside the agreed outcome. The shipped command-line flags do not send container
edits, so silently changing isolation through the original omission required a hand-built protocol request.

When terminal attachment debug tracing is enabled, a host-supplied tab name is bounded and escaped before either the
start or end event logs it. The ordinary agent-terminal field remains unchanged for the desktop smoke check. This trace
is off by default. The specification now states that the escaping rule covers all peer-supplied text the helm logs or
retains, including identifiers, rather than errors alone. No new validation framework was added.

The terminal recovery-gap caveat now explicitly says that a multi-line paste into a shell may run line by line rather
than arrive as one block when lost output leaves the terminal's input modes wrong. This names the consequence you
accepted; it adds no recovery behavior or mitigation.

The old-wrapper finding was removed under your decision to leave old templates unchanged. Wrappers saved by 0.30–0.32
can still make a newly supplied prompt be parsed as shell syntax. Current wrapper editing behavior is unchanged. This
outcome is a discard, not a repair.

The two code outcomes stayed within their size gates: 68 and 43 changed source, test and documentation lines, excluding
the ledger, feedback removal and changelog fragments. Each of the four outcomes records its change ID, bookmark and PR
URL, and removes its own feedback file and index entry. There is no schema or protocol change.

## Open questions and possible follow-ups

None. The remaining paste-after-gap and old-wrapper behavior above are the recorded decisions, not unfinished fixes.

## The PRs

- [#3035 — show container settings on template approvals](https://github.com/scode/farhelm/pull/3035/changes), draft.
- [#3036 — escape tab names in attachment logs](https://github.com/scode/farhelm/pull/3036/changes), draft.
- [#3037 — name the paste risk after a recovery gap](https://github.com/scode/farhelm/pull/3037/changes), draft.
- [#3039 — retire the old-wrapper prompt finding](https://github.com/scode/farhelm/pull/3039/changes), draft.

## Checks run, reused and skipped

The runtime commands use the test recorder with pinned nextest 0.9.148, four global slots, zero retries and tmux marked
unused. These units do not need a tmux server.

- `cargo nextest run -p farhelm-ui --lib -E 'test(approvals::tests::a_template_)'`, recorder
  `32812d33-4d3f-4510-8b7a-3869f1108eec`: five passed, including configured container values, no token file, removal and
  omission, plus existing template card cases. The 853 other tests were outside the selection. Recorder and child exited
  zero with complete output and JUnit; no runtime substrate skip occurred.
- `cargo nextest run -p farhelm-helm --lib -E 'test(/^terminal::tests::/) | test(/^client::tests::/)'`, recorder
  `0f5d61b2-9a40-437b-85b4-cd1cf8f5dda7`: all 103 passed, including exact escaped tab fields on both trace events and
  the existing attachment integration that checks the plain agent field. The 1,385 other tests were outside the
  selection. Recorder and child exited zero with complete output and JUnit; no runtime substrate skip occurred.
- `cargo clippy -p farhelm-ui --all-targets -- -D warnings` and
  `cargo clippy -p farhelm-helm --all-targets -- -D warnings` passed.
- `cargo fmt --all -- --check`, dprint on changed Markdown and `python3 releasing/check-changelog.py format` passed. The
  isolated pinned interpreter's `python -B scripts/check-test-sleeps.py` inspected 412 delays with no missing reasons.
  `python3 scripts/check-agents-md.py` reported zero errors and three existing unrelated reference warnings.
- Reused across ledger-URL amendments and the later prose-only outcomes: the card run's source on base `7b826e40`,
  fingerprint `262f1d3965b08e4fef5e57fc48cadecb35491d7cc33ac10c2d2f813e08f8954a`, and the reviewed UI lint result. The
  tested card code is unchanged. The helm run covers the PR2 working source on base `5bd1a1f1`, fingerprint
  `f89cbd591a772a09090830938687c5218c7e077dce7ec2cdf9ba5300a78155fb`; later changes only add ledger URLs and prose, so
  its executable coverage still applies. Upstream changes through `9949406d` contain only queue state, a different
  plan's answer and a report; they change no code, specification or dependency used here, so no rebase or repeated
  runtime check was warranted.
- Skipped: the full Rust workspace, browser suites, desktop runtime and desktop smoke. The executable changes are
  confined to card data rows using the existing renderer and the debug trace's tab-name field. Focused card tests and
  actual trace-event assertions cover those changes; the existing relay integration checks the unchanged plain agent
  field. No CSS, browser interaction or desktop lifecycle changed. The two prose/bookkeeping outcomes need no runtime
  tests. No hostile-host exploitation or real shell recovery experiment was performed.

## Review gate's outcome

Each PR passed its required single fresh-context gpt-6.1-sol review at high effort, with no findings. The changed code
and tests received a separate documentation pass. All commit and PR wording passed fresh cold reads; the discard message
was clarified to name the actual prompt-handling concern. No scope departure or unresolved review finding remains. The
four PRs are open drafts in the required linear order; nothing was marked ready or merged.
