## What this was about

A wrapper that sets an agent's API endpoint or other launch settings previously had to be entered as a custom command.
Farhelm then lost the agent choices it normally composes and could not treat that session as the selected agent
throughout its lifecycle. You chose a simpler contract: Farhelm puts a user-supplied command prefix before its normal
agent command and trusts the user to make the wrapper pass through what the agent needs.

The two draft PRs add that prefix to agent launches, the launcher, templates and quick launch, and agent command-line
requests. New, Clone, Restart with and Replace with carry the selected prefix. Plain Restart and resume use the
session's saved command. The launcher remembers the last successful user-launched prefix separately for each agent type;
a template can leave that memory alone, replace it, or explicitly remove the wrapper. The covered TODO entry is removed.

## Things you should know

Farhelm splits the prefix into words using shell-style quoting and escaping; it does not run a shell or expand
placeholders. Farhelm does not inspect the wrapper to prove what it does. The wrapper must pass Farhelm's arguments and
environment through to the agent. Conversation capture keeps each agent's existing process rules: Claude and Grok permit
an executing wrapper or one resident wrapper level, Codex permits ancestry above its innermost `codex`, and OMP requires
the wrapper to replace itself with the agent process.

The prefix is visible in the launch command and process arguments. Agents can also read a template's prefix in the
templates listing, so it must not contain secrets. The launcher and Restart with show unsafe characters from a saved
prefix as escaped, read-only text until the value is cleared; the raw saved words remain the launch source of truth.

On a host that asks before YOLO launches, a prefix supplied by an agent through a command flag, Clone or inherited
session needs an Allow/Deny approval card even if that host normally runs Farhelm commands without asking. The card
shows the agent-supplied prefix; Always allow is not offered. A prefix from a template named in the request counts as
user-configured. YOLO permission itself remains refused on such a host. Managed-checkout retries retain the original
request's launch and approval facts, so an interrupted request cannot silently turn an approval into a different wrapper
launch.

The protocol and helm database schema were bumped. As you accepted during planning, an older helm cannot read the new
database and an older peer cannot speak the new protocol; upgrades must keep the helm and peers aligned. Main gained
host appearance preferences while this stack was in progress, so the wrapper preference migration was placed after those
migrations and its combined read/write path was retested.

## Open questions and possible follow-ups

No decision is pending for this plan. If managed-checkout retry handling changes again, a direct test of a second
interruption after Allow would strengthen the existing persistence and replay tests. The documentation prose was
updated; existing screenshots were not recaptured in this plan.

## The PRs

- [#2271 — run agent launches through wrapper prefixes](https://github.com/scode/farhelm/pull/2271/changes), draft, head
  `96709438b62e85975cf3c6124bb8388dc1bd89bf`, based on main.
- [#2272 — set wrapper prefixes in launcher and templates](https://github.com/scode/farhelm/pull/2272/changes), draft,
  head `8cb505925f79e904700de9eeffacb142479e011a`, based on #2271.

## Checks run, reused and skipped

Focused Rust tests ran through the recorder with pinned nextest and tmux, four slots and zero retries. The redacted
child commands and results were:

- `cargo nextest run -p farhelm-proto -p farhelm-helm -p farhelm-ui -p farhelm-supervisor --lib -E 'test(wrapper) | test(prefix) | test(schema_58) | test(only_an_explicit_selection)'`:
  47 passed, run `984978d8-945e-4b33-aba4-1270b61b9ee7`.
- `cargo nextest run -p farhelm-helm -p farhelm-supervisor -p farhelm --lib --bin farhelm -E '<four clone, inherit, OMP and CLI wrapper cases>'`:
  four passed, run `bbe07e17-bb22-4b78-9357-bb836e2ca201`.
- `cargo nextest run -p farhelm --test e2e -E 'test(=wrapper_launch::a_structured_agent_wrapper_forwards_start_resume_and_environment)'`:
  two reported passes, run `494e03d4-d687-4d03-ba81-aa05f3b91a61`.
- `cargo nextest run -p farhelm-supervisor --lib -E 'test(fresh_fingerprint_binds_and_recovers_the_resolved_launch)'`:
  one passed, run `07387e7c-f820-42d3-be96-e34ee37116cb`.
- `cargo nextest run -p farhelm-helm -p farhelm-supervisor --lib -E '<three guarded managed-checkout retry cases>'`:
  three passed, run `4adc86a3-7bea-44ea-8474-534f4011d9e4`.
- `cargo nextest run -p farhelm-proto --lib -E 'test(checkout_recovery_snapshot_and_fingerprint_roundtrip)'`: one passed
  after the wire representation correction, run `7ace11e7-6a95-49aa-b956-7c6eb1888030`.
- `cargo nextest run -p farhelm-helm --lib -E '<three wrapper-memory and host-appearance migration cases>'`: three
  passed after rebasing onto main `262c545f`, run `4fb8900e-8167-47c7-a402-50e3e564492b`.
- `cargo nextest run -p farhelm-ui --lib -E '<three wrapper launcher and template cases>'`: three passed on the second
  PR's assembled UI, run `5c1a41d2-f9e0-46e0-b294-6c6e71ceed0b`; its selection label said four, but the report selected
  three.

All selected Rust reports were complete, with no selected runtime early return. The recorder fingerprints identify the
exact working tree for each run, including uncommitted changes. In the Rust order above, the first four were based on
main `be6e11d8`, with source fingerprint prefixes `8d5ea5131314`, `7f891c6d1c02`, `c5f8d6127c93` and `793738851c63`; the
guarded retry and wire runs were based on `82dbf720`, fingerprints `28d8b57afa98` and `c7763afc2435`; the migration run
was based on main `262c545f`, fingerprint `86cec003f058`; the UI run was based on pushed #2271 head `96709438`,
fingerprint `69549df7a423`. Full fingerprints are in each run's retained manifest. The first three commands ran before
the guarded retry correction; their launch, clone, OMP and CLI behavior was unchanged by that correction and by the
later host-appearance rebase. The fingerprint, retry and wire commands ran after their respective corrections. The
migration command specifically covers the host-appearance integration. The UI units ran before the second PR's
saved-value and template-save corrections; the final browser run below covers both corrections. The second PR did not
change backend launch, guard or persistence behavior, so those first-PR results continue to cover the delivered stack.

Browser checks used `npx playwright test -g '<wrapper launcher, agent switch and quick-launch cases>'`: six passed
across Chromium and WebKit on the first PR's UI candidate, run `f9df38ee-5e4c-4665-8dd5-ea20327db77f` (based on
`be6e11d8`, source fingerprint `9c492a5e8d2d`). After the second PR's review corrections,
`npx playwright test -g '<wrapper launcher, invalid dormant draft, agent switch, quick launch and unsafe Restart with cases>'`
passed ten cases, five per engine, run `68d28764-c493-4dff-a657-4c8f621d95d6` (based on #2271 head `96709438`, source
fingerprint `f73b3282de96`). The latter report had zero failures and skips, complete output and fixture cleanup. It
supersedes the earlier UI result for the final behavior. Only a syntax-level collapse of a nested guard followed that
browser pass.

On the final combined tree at #2272 head `8cb50592`, `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
`cargo check -p farhelm-ui --features desktop`, `dprint check`, `python3 scripts/check-agents-md.py`,
`python3 releasing/check-changelog.py format`, the isolated `python -B scripts/check-test-sleeps.py`, `bun run build`
for the docs site, and `git diff --check` passed. Shipped-binary Clippy was rerun on this exact head during report
preparation; the other checks passed on the final source tree before the commit, with no subsequent behavioral edit. The
command-line and web builds needed for browser execution passed. Earlier command, fixture and lint failures were
retained in the private working log and corrected before the listed passes; none is counted as a passing check.

The full Rust, browser, desktop and installer batteries were skipped. The focused runs cover launch composition,
approval and retry provenance, persistence, UI choices and browser behavior; the broader batteries would not target an
uncovered risk in this diff.

## Review gate

Each PR passed its required fresh-context gpt-6.1-sol high review. The first review found a managed-checkout retry path
that could lose the original approval boundary; the guarded recovery was corrected and reviewed again. A later error
message was corrected. The second review found an unsafe saved-prefix display and a command-template save blocked by a
dormant invalid agent prefix; both were corrected, retested on both browser engines and confirmed resolved. Commit and
PR wording passed the separate cold-read gate. Both PRs remain drafts for the landing monitor.

### Landing

Landed on its own in round 94, as #2271 and #2272, merged bottom-up, with several fixes made during the landing (below)
and one decision left for the maintainer.

#### What landed since the stack was based, and whether it interacted

Only plan-queue bookkeeping reached main between this stack's base and its landing, and no other plan landed in this
round; the landing rebase was clean. The executor had already rebased over the changes main gained while it worked,
among them the command that sends a message to a running agent, the managed-agent trust settings, the dialog focus
change and the host colors. The landing monitor read the stack against all of them, and a fresh-context reviewer did the
same independently. Neither found a conflict:

- Sending a message to a running agent chooses its delivery by the agent type the session was launched as, not by the
  process, so a wrapper does not change it.
- Trust arguments are placed from the end of the command, and Farhelm's own placeholders are refused in a prefix, so the
  trust settings and wrappers compose.
- The launcher's and Restart with's new prefix field behaves under the dialog focus rules like the other fields.
- YOLO, command and older-style launches stay refused after an Allow on a host that asks before YOLO launches; an Allow
  is bound to its card, and Always allow is refused for these cards on both the helm and the app. Clone and inheriting a
  session's launch always force the card; a managed-checkout retry is judged on the original request's launch and
  approval, not on today's template. Status detection and conversation capture follow each agent's existing rules for
  wrapped processes.

#### Upgrade

#2271 raises the protocol from 57 to 59 and the helm database from schema 59 to 60. The protocol skips 58, most likely
because the executor bumped it twice inside the unmerged PR (once for the new launch field, once for fields added after
review); one bump would have done, and the skip is harmless since none of these versions has shipped (v0.29.0 carries
protocol 51). The next release already requires remote hosts to be updated, and an older helm refuses the newer
database. The pending agent-quick-launch-templates plan also changes the protocol and will need 60.

#### Fixes made during the landing

The whole test libraries, rather than the executor's selections, found five failures in the stack itself, all fixed
before merging:

- The new remembered-wrapper column sat in a different position in a newly created database than in an upgraded one, so
  the test that compares the two failed, and three tests that rewind a database to an older schema did not remove it.
  The new column now comes last in a new database, as every column added since that table was created does, and the
  rewinds remove it.
- A test of the agent instructions still expected the usage line for creating a session without the new wrapper option.

From the reviews:

- The rule that decides which approval cards force a visible Allow ended in a catch-all that answered "no". The pending
  agent-quick-launch-templates plan adds a card that carries everything a template write carries, and through that
  catch-all it would have skipped the forced Allow for a wrapper prefix. Every card kind is now listed, so a new one has
  to decide. That plan's executor should know its card must force Allow when it carries a prefix and show the prefix.
- SPEC.md and the Approve what agents do docs page described the forced Allow only for launch cards. An agent's template
  write that leaves a wrapper prefix in the template also always shows a card with Allow or Deny only, whatever the host
  allows without asking, because launches from that template later count the prefix as the user's; both now say so.

#### A decision for the maintainer

When an agent's request names a template and also asks for a different agent type, the helm keeps the template's wrapper
prefix for the new agent type, while the session launcher, on the same change, swaps in the prefix remembered for the
new agent type. So an agent running `farhelm agent create --template wrapped --agent codex`, where `wrapped` is your
Claude template with a wrapper, runs that Claude wrapper in front of Codex, and because the words came from your
template rather than the agent's own flags, no card is shown even on a host that asks before YOLO launches. It can only
run wrapper words you saved, but it lets an agent pair your wrapper with a different agent, and it contradicts SPEC.md's
rule that applying a template means the same as making those edits by hand.

The options: (a) clear the prefix when the agent type changes in the helm, a small change that matches what the launcher
does when no prefix is remembered for the new agent; (b) carry a template's prefix across an agent change in both
places, launcher included. Either way SPEC.md should say which. The landing monitor recommends (a), since a wrapper is
usually written for one agent.

#### Possible follow-ups

- The changelog entry leaves out caveats worth keeping at release time: the wrapper must pass Farhelm's arguments and
  environment through, OMP needs the wrapper to replace itself with the agent, and the prefix must not hold secrets.
- If managed-checkout retry handling changes again, a direct test of a second interruption after Allow would strengthen
  it, as the report says.

#### Checks

- Run now, on the rebased stack with the landing fixes (the landed diffs are identical to it):
  `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the isolated
  test-sleep check (304 delays, none without a reason), and `dprint check` on the changed docs; through the test
  recorder with pinned nextest and tmux, four slots, no retries, the whole protocol, helm, supervisor and UI libraries,
  the `farhelm` binary's tests, and the end-to-end wrapper, structured-launch, Restart with, managed-checkout and
  agent-listing tests: 3270 passed, run `30c352ba-b443-49c0-8e86-8b6bba3d8c0c`; the UI JavaScript harness, run directly
  rather than through the recorder: 294 passed; then fresh `cargo build` and web builds and `npx playwright test` on
  `launcher-tabs.spec.ts`, `quick-launch.spec.ts`, `restart-with.spec.ts`, `templates.spec.ts`, `clone.spec.ts`,
  `launcher-enter.spec.ts`, `approval-layout.spec.ts`, `yolo-guard.spec.ts` and `remembered-permissions.spec.ts` on both
  engines, one worker, no retries: 332 passed, run `e6265481-d123-44a2-aeed-cd98df1cc2b1`.
- Retained: the first whole-library run with the five failures above, run `f04d9aee-fede-43cb-9954-0cab2d28a68d`.
- Reused: the executor's website build and changelog format check; the landing's docs edit adds no link.
- Skipped: everything else the report skipped, for the reasons it gives.

Nothing else in the executor's report became untrue at landing, apart from the checks and fixes above and its statement
that no decision is pending: the landing found the one above.
