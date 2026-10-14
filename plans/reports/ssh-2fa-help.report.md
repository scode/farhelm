### What this was about

Farhelm connects to hosts without answering SSH prompts. A host that asks for a second factor or other approval on every
login can work only when its two connections are started by hand, one for sessions and one for setup and updates. Users
previously had to know Farhelm's private connection paths and settings. The maintainer chose a Help menu dialog with
exact commands and copy buttons, also reachable before adding a host.

### Things you should know

**? → ssh needs 2FA?** now lists both commands for each registered SSH host. The menu order is Send feedback,
Documentation, Release notes, SSH help, Welcome, then Check for updates where offered. The dialog explains which machine
and account must run the commands, asks users to rerun a dropped connection, and recommends asking an agent to automate
them from a screenshot.

Add host opens the same dialog with the typed destination, preserving its draft and keyboard isolation on return. Empty
destinations get guidance; invalid ones are refused. Opening help neither adds a host nor starts SSH. The helm composes
the commands from its own path, existing connection overrides and quoting, including disabling port forwards that a
later client cannot cancel. Relative state paths preserve the helm's working directory. If a path is too long, contains
literal `${`, or is not UTF-8, the dialog explains the affected connection instead; the sessions command remains
available when only setup cannot fit.

The ordinary SSH transport is unchanged. This remains best-effort support for hosts requiring interactive approval. The
docs pages [Add a remote host](https://farhelm.io/docs/get-started/add-a-remote-host/) and
[Send feedback](https://farhelm.io/docs/using/send-feedback/) now explain the help entry; those public pages update only
with a separately requested website deployment.

### Open questions and possible follow-ups

No maintainer decision blocks this work. The real OpenSSH check used test-key authentication on private loopback, not an
actual second-factor service or a real remote host. Browser coverage exercises both engine families; the native desktop
clipboard bridge was compiled and its existing implementation reused, not manually tested on a Mac.

### The PRs

- [#2018: commands for hosts needing SSH approval](https://github.com/scode/farhelm/pull/2018/changes), one coherent
  draft containing the authenticated help, UI, docs, changelog fragment and covered TODO removal. No ready or merge
  action.

### Checks run, reused and skipped

- Run: recorded
  `cargo nextest run -p farhelm-helm -p farhelm-ui --features farhelm-ui/desktop --lib -E 'test(ssh::) | test(ssh_help::) | test(app_bar::tests::the_help_menu) | test(modal_isolation::)'`,
  23 passed, 1703 filtered out; run `a14686f3-544a-4a96-a048-b3e049ac9bb2`. Pinned nextest and tmux, four slots and zero
  retries. No runtime substrate skips. This covers command quoting, unavailable paths, relative working-directory
  context, protected read-only help and desktop menu order.
- Run: final full-vector assertion with filter
  `test(=ssh::tests::manual_master_commands_preserve_the_connection_boundary)`, 1 passed, 1093 filtered out; run
  `5f6adc30-9597-4627-b7f1-d5afdafba3f7`. This strengthens the correspondence between the manual commands and Farhelm's
  ordinary SSH argv without repeating unrelated cases.
- Run: recorded Playwright `tests/ssh-help.spec.ts tests/feedback.spec.ts tests/tooltip-coverage.spec.ts`, 34 passed
  across Chromium and WebKit, run `819ac326-ee41-4fbd-af47-088e9695e48c`; then `tests/ssh-help.spec.ts` after scoped
  layout polish, 6 passed, run `390f43c4-b935-4291-8360-3a6d70f35266`. Both use the recorder's Playwright mode, one
  worker, zero retries and pinned tmux. Fixture shutdown completed. Copy uses a controlled clipboard sink; no live
  feedback was sent. The optional unreachable sample remote is not remote-host coverage.
- Run: one-time recorded OpenSSH check, run `fe260f2e-88cf-4050-9801-2a9f602ef143`, with OpenSSH 10.2p1, a private
  loopback sshd, test keys and `MaxSessions 1`. Requested actual commands from the built helm, started both masters, and
  verified each with `ssh -O check`. Ordinary-client argv, whose exact correspondence is pinned by the unit test, ran
  `true` over each socket without a fresh authentication: accepted-key count stayed 1→1 for sessions and 2→2 for setup.
  Both masters and the private services were stopped afterwards.
- Run: `cargo clippy -p farhelm-helm -p farhelm-ui --features farhelm-ui/desktop --all-targets -- -D warnings`,
  including after production corrections; `cargo clippy -p farhelm --bins -- -D warnings`; native CLI/fixture build and
  final release web build. The desktop feature compiled in the affected all-targets lint. `cargo fmt --all -- --check`,
  targeted Markdown `dprint check`, `git diff --check`, changelog format lint, the isolated
  `python -B scripts/check-test-sleeps.py` (299 delays, zero missing reasons), and website frozen install/build passed.
  A separate documentation pass covered all touched files and final lifecycle/relative-path contracts.
- Run: recorded visual inspections `e18e057b-09b5-417b-9f74-f482103830c6` and `3c55cd79-8bda-4ec4-9f8c-de9cdc116bbe`.
  The first showed excessive spacing in the inherited narrow dialog; the final inspected capture shows both commands and
  copy controls in the approved wider shape, with dismissal reachable. Captures remain private.
- Reused: unaffected checks across the careful rebase from `9bad89a9` to `55aec530`. The only incoming change adds an
  unrelated TODO item. Its mechanical conflict was resolved by preserving it and removing only this plan's entry.
  Production code and tested behavior did not change. The final focused browser run covers the subsequent CSS
  adjustment; the earlier broader run remains coverage for other unchanged surfaces.
- Skipped: full workspace and browser batteries, hosted CI, installer/release gates and Mac desktop runtime. The
  affected packages, protected endpoint, transport boundary, shared modal lifecycle and engine integration have targeted
  evidence; no installer, protocol, database schema or release pipeline changed. No executable documentation examples
  need doctests. No production signing, deployment, real host or real credentials were used.

The retained first Rust run `4ee52656-c58a-4500-adc0-3d3d3d2a9ffe` has 20 passes and one failure: the new test omitted
the accepted Host header, so a DNS-rebinding refusal occurred before its expected device-auth refusal. The fixture
premise was corrected; later passes are separate observations. This was a test error, not a production authentication
change. Initial setup refusals for container evidence ownership and recorder argv occurred before execution; two offline
web-build attempts could not obtain the pinned build tool. All remain recorded separately from runtime passes.

### Review gate outcome

The required fresh gpt-6.1-sol reviewer at high effort found three issues: restoring focus did not actually reinstall
the still-mounted parent modal's isolation; relative state paths did not preserve the helm's working directory; and the
shared flex style overrode the parent form's hidden attribute. All were corrected with direct regressions and
independently rechecked. The same reviewer accepted the final authentication-fixture/full-argv assertions and scoped
visual CSS, with no unresolved findings or scope excess. Source review did not execute tests or independently verify
runtime evidence.

Implementation stayed in the main agent under the plan's no-workhorse requirement. Required source review used
gpt-6.1-sol/high, wording reads used gpt-6.1-sol/medium, and report delivery receives the separate native own-model cold
read. Private review records retain decisions and outcomes. Exact native and main-agent usage is unavailable; external
counters are retained without assuming model attribution or billing totals.
