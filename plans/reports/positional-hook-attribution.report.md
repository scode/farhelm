# Attribute Grok and Codex reports to the process that ran the hook

## What this was about

Grok's installer points its launcher at a versioned executable. Farhelm used the executable name to recognize Grok's
conversation reports and rejected them on that install, so **Restart** could not resume the conversation. Codex depended
on its executable name too.

The maintainer chose the rule already used for Claude, Muse, and Antigravity. Determine which process ran the hook, then
tie the conversation to the process whose report was first accepted. A process above it in the same launch can correct
that choice by reporting its own conversation. Farhelm passes `--no-leader` when it starts Grok, which keeps Grok's
backend in the session. OMP keeps its separate launch check because another interactive OMP can send a report from
inside the session before the original process sends one.

## Things you should know

- A child can report first if the parent's first report is late or was lost. This includes a shell running a Codex hook
  before Farhelm has recorded a reporter. A later qualifying report from a recorded ancestor changes the selection and
  posts a correction notice. For Grok, that report must be a `SessionStart`. A corrected Grok selection needs a verified
  record before **Restart** can resume it. If the saved record is missing or mismatched at Restart, Farhelm keeps the
  selected conversation and withdraws the offer.
- The plan accepted an upgrade gap for Grok sessions that already had a conversation but no recorded reporter. A nested
  Grok's newer `SessionStart` can select a different conversation until the parent next reports its selection. Codex
  keeps any process and ancestors it already recorded. A child whose report still shows the recorded parent cannot
  replace it. Both Grok and Codex can release a pin for a process that has exited and is absent from the incoming
  report's ancestry. The new report still needs to pass its integration's checks. In particular, Grok's `--no-leader`
  check excludes the transient shell reporters that Codex can accept.
- The old Codex name rule could mistake the hook executable itself for the agent if someone named that executable
  `codex`. The shared rule starts above the hook invocation. The existing Codex integration tests continue to cover the
  supported native, wrapped and nested cases.

## Open questions and possible follow-ups

None identified.

## PRs

- Share the rules for retaining or replacing the saved reporting process.
  [#4724](https://github.com/scode/farhelm/pull/4724/changes)
- Restore conversation tracking and Restart for versioned Grok installs.
  [#4725](https://github.com/scode/farhelm/pull/4725/changes)
- Track a Codex conversation through renamed executables and handle a child-first shell report.
  [#4728](https://github.com/scode/farhelm/pull/4728/changes)
- Document the default for new integrations, OMP's exception, and accepted gaps.
  [#4729](https://github.com/scode/farhelm/pull/4729/changes)

## Checks run, reused and skipped

- `python3 -B scripts/run-harness-drift.py --harness grok --harness codex` passed at `d0262280` on Linux. Both updater
  checks reported already current. Grok 1.0.92 (run `aef9459f-366a-468c-ab43-34a529a32717`) and Codex 0.171.0 (run
  `89856f46-1765-4e55-8748-f238502e25bf`) each passed first report, Restart, recall, and a fresh accepted report for the
  resumed launch.
- The recorder ran pinned `cargo nextest run -p farhelm-supervisor --lib -E <selection>` with four slots and zero
  retries. These are the tested trees and exact selections.
  - `c3cfa107`, 138 passed with `test(/^procs::|::claude_pin::|::report_pin|::codex/)` (run
    `2f4c70e3-cc02-445d-b2a4-21aa9348f267`).
  - `c03b11cb`, 117 passed with `test(/^(procs|agent_kind::grok|service::notifications)::|::report_pin|::claude_pin::/)`
    (run `81807dc3-9852-49d2-b1b6-c2ec3102bd8b`).
  - `e7490f86`, 123 passed with
    `test(/^(procs|agent_kind::codex|agent_kind::grok|service::notifications)::|::claude_pin::|::report_pin/)` (run
    `ca100821-179a-449c-a35f-9d32e92fafd6`).
  - `20d93c69`, three passed with `test(a_renamed_codex_image_can_emit_the_hook) | test(/^service::notifications::/)`
    (run `acb50b8f-0724-4bf6-af57-70bdd8a12fc9`).
- After `cargo build --locked -p farhelm-fixtures`, recorded `cargo nextest run -p farhelm --test e2e -E <selection>`
  passed 35 at `c3cfa107` and all 36 at `20d93c69` with `test(/^(codex_identity|hook_identity)::/)` (runs
  `000ca7ad-313a-4d85-9e83-f683a4cbeb4d` and `792d2340-a9db-4914-ba22-ea10dd7b4af4`). At `c16022a5`,
  `test(/^hook_identity::/)` passed all 31 after strengthening the nested Grok case (run
  `5bc73882-cd52-466f-b8ef-ef97e41b1357`). The recorded output showed no runtime skips.
- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, the annotated test delay check
  (`python -B scripts/check-test-sleeps.py` in the isolated pinned environment), and
  `python3 releasing/check-changelog.py format` passed. `dprint check` passed on the changed Markdown.
  `bun install --frozen-lockfile` and `bun run build` validated the docs website. The Codex, Grok, wrapper and hook
  pages were checked in that build.
- Reused the Clippy checks from `e7490f86` after the Codex follow-up changed diagnostic text, tests, and comments. Reran
  the targeted runtime checks for that follow-up. Reused the deterministic runs through `20d93c69` at `d0262280` because
  the later changes were a fixture's failure messages and documentation. Main advanced during validation and host
  auto-update landed during the report review. Its changes are to the helm's host connection and update flows, which are
  separate from local conversation reporting and Restart. The stack kept its tested base. Skipped the full workspace
  test suite because the change stays in hook admission and its docs. The recorded live checks ran on Linux.

## Review gate

All four PRs passed separate source and wording reviews. The Grok review strengthened the test so ordinary timestamp
ordering could not hide a missing parent pin. The Codex review corrected the notice for a shell reporter, preserved the
description of existing pins across upgrade, and tested a renamed executable below a resident wrapper. The final review
checked the documented default and OMP exception against the implementation.

### Landing

#### What landed with it

The plan's four PRs (#4724, #4725, #4728, #4729) and one landing fix (#4734) landed second in a round with the
custom-agents plan, which landed just before it (with its own landing fix). Since the stack's base, main had also gained
the automatic host update fix (host-auto-update-after-2fa), which touches nothing this plan changed.

The two plans were built at the same time and both reshape how the supervisor decides which process sent a conversation
report, so every one of this plan's commits conflicted with custom agents when stacked on it: the attribution code, a
code comment that maps where each agent's behavior lives, and SPEC.md's Codex paragraph, which custom agents put a new
paragraph in front of. The landing kept both sides everywhere (this plan's wording for the attribution module's opening
line, since custom agents' "for agents with no executable identity rule" stopped being true once Grok and Codex use it).
Two interactions produced no textual conflict but would not have compiled, and the landing fixed them inside this plan's
commits. Custom agents gave the shared attribution function a second argument saying which reporter command to expect,
so this plan's new Grok and Codex callers and 18 of its Codex tests now pass the built-in hook command. And both plans
added a correction notification kind to a fixed-length list whose declared length the merge left at 12 for 13 entries.

The landing's reviewer for this plan compared its diff before and after the rebase: the only differences are those
resolutions and fixes. It found that custom agents' admission is unchanged by this plan, that Grok and Codex need none
of custom agents' checks (launch validation refuses the custom reporting setting on any other agent type), that the
shared code deciding whether a recorded reporting process still holds the session behaves exactly as the old per-agent
code did, and that every process chain the old Codex name rule accepted gives the same reporting process under the new
rule.

#### Fixes made while landing

- #4734 updates descriptions of the rule that had not kept up. SPEC.md still said Grok gives up capturing a conversation
  when a resident Bash sits above its wrapper with another wrapper below; it now says Claude, Muse, Codex, Grok,
  Antigravity and custom agents tolerate it and only OMP refuses. SPEC_impl.md's default attribution rule and its list
  of agents that keep the recorded reporting process, and two code comments listing which agents share that code now
  name custom agents and the shared check.
- **The report and the Grok docs said Farhelm always passes `--no-leader`.** It adds the flag only when it composes a
  Grok launch from the launcher's Grok agent type. A custom command that declares Grok must include it, because Farhelm
  refuses every report from a Grok started without it (a requirement that predates this plan). #4734 says so in the Grok
  setup page and the hook injection page; its commit message wrongly says the refusal is new.
- #4734 also rewords Codex test comments that still described the removed executable-name check as the production rule.

#### Checks

The two plans touch the protocol, the supervisor's report attribution, the helm's launches, registry and CLI, the
desktop window's Settings and launcher, the installer's launcher script and the docs site, and each executor ran
filtered selections. So the landing ran the whole workspace suite, every lint, the desktop app's own tests (the app
embeds the changed window), the installer test and the end-to-end smoke on the two plans stacked on main, with every
reference picture, the website build and the source checks locally. The worker runs were on a separate arm64 Linux test
worker; the rest ran locally on x86_64 Linux.

On both plans stacked on main, with the conflict resolutions and compile fixes:

- `cargo nextest run --workspace --exclude farhelm-desktop`, on the test worker (recorder run
  `adc951be-a12e-44f5-9490-806b28f11ad4`): 8508 of 8511 passed and none printed a runtime `SKIPPED`. Two of the failures
  were the helm's schema 26 migration tests that custom agents broke (fixed in #4733, see its notes). The third was the
  helm's Tensorlake test of carrying a Codex sign-in choice into environment setup, which found a Codex credential
  marked cleared where it expected none. Neither plan touches that flow, and the same test failed once during the
  host-auto-update-after-2fa plan's execution on a tree without either plan. It passed 20 of 20 repeated runs alone on
  this tree, locally on x86_64 rather than on the arm64 worker where it failed (recorder repetition batch
  `2ebb6eaf-0b96-4280-9ac5-17a22c8ac47a`), and again on the worker in the run after the fixes below. It looks like a
  latent flake that shows only under load. The landing did not record it in FLAKES.md, which takes its own change;
  recording it is a follow-up.
- `cargo nextest run -p farhelm-desktop`, on the test worker (recorder run `8dce2b68-35aa-4ad4-a8ee-e51508e3ac5f`): all
  184 passed.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings` and the desktop app's
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings` with and without `--features agent`, on the test
  worker: passed.
- `bash scripts/test-install-sh.sh --farhelm target/debug/farhelm`, on the test worker, run directly as the validation
  guide lists it rather than through the recorder: 547 checks, none failed.
- `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run `bd07df46-6f7f-449c-89cf-6fe615c8bf44`): of
  its 50 steps, 49 passed and one was skipped as always on Linux (paste, which the headless Linux desktop session the
  smoke runs in cannot provide).
- `python3 scripts/check-gui-goldens.py`, locally (recorder run `9f6e7283-22b0-4a28-8465-fa845314e194`): every reference
  picture matched, including custom agents' new launcher, Settings and approval scenes.
- `python3 -B scripts/test-gui-agent.py`, locally (a self-test the validation guide lists without the recorder): failed
  on the new custom launch type (fixed in #4733; see the custom-agents landing notes).
- `cargo check -p farhelm-supervisor --tests`, locally, at each of the positional plan's first three commits and at the
  top of the stack: compiled, so each of that plan's commits builds on its own after the landing's fixes.
- `python3 -B scripts/test-gui-screenshot-batch.py`, `sh -n scripts/install.sh` with `shellcheck`,
  `python3 scripts/check-agents-md.py`, the website build (`bun install --frozen-lockfile` and `bun run build`),
  `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py` (in its pinned interpreter), `dprint check` and
  `python3 releasing/check-changelog.py format`, locally: passed.

After the landing fixes (#4733 and #4734):

- `cargo nextest run -p farhelm-helm -p farhelm --lib --test e2e` selecting the helm's library and the agent listing
  end-to-end module, whose test checks the new refusal, on the test worker (recorder run
  `89e4e6ca-903f-4197-9fa9-479aada524d8`): all 1891 passed, including both migration tests and the Tensorlake test, and
  none printed a runtime `SKIPPED`.
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`, on the test worker:
  passed.
- `python3 -B scripts/test-gui-agent.py`, `sh -n scripts/install.sh` with `shellcheck`, the website build,
  `cargo fmt --all -- --check`, the sleep check, `dprint check` and the changelog format check, locally: passed.

Reused for the fixes: the rest of the workspace suite, the desktop tests, the installer test, the smoke and the
reference pictures above, since the fixes change one helm refusal, a test fixture, two scripts' launch type lists, a
supervisor message and documentation. Skipped: macOS, where no Mac was used, so custom agents' new Settings and launcher
screens are unchecked there; the real-agent drift tests for Grok and Codex, which the positional plan's executor ran and
nothing at landing changed their behavior; CI, which runs only on demand.

#### Things you should know that the report does not say

- **After Codex exits under a wrapper that leaves a shell, that shell can now report a Codex conversation.** With a
  command such as `codex; exec bash`, the recorded Codex process has exited and the shell does not descend from it, so a
  report from the shell is accepted if it carries a valid transcript or is a `/clear`. Before, it was refused because no
  Codex process was in its ancestry. This follows from the decision to let a shell report first, and it is not a
  security issue (same user, same inherited credential), but SPEC_impl.md does not describe this after-exit case.
- **The Codex and Grok changelog fragments do not mention the new correction notice.** A script or nested agent that
  reports first can briefly supply the conversation until the agent's own report switches it back with a notification;
  before, that first report was refused silently. Release-time curation can add the caveat.
- When Farhelm cannot read whether the recorded Grok process is alive (an OS read error), a nested Grok's end-of-turn
  and prompt reports log a refusal where Claude, Muse and Antigravity stay quiet. Cosmetic.
- The Codex attribution unit tests now call the shared function directly, so they would not notice if Codex admission
  moved to a different rule.
