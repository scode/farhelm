## What this was about

Agents could create and edit launch templates, but a person still had to use the Templates dialog to give one the ⚡
quick-launch mark or a number key. The agent's template listing did not show those assignments. You chose to let agents
set and clear both through the existing template commands, to move an occupied shortcut as the dialog does, and to name
the previous holder on the approval card.

The two PRs add those commands and listings. A shortcut change stays on the template write's single approval card. The
helm checks the resulting template before assigning ⚡, and commits an approved template and its bindings together only
while the template and the full shortcut map still match what the card showed. A key may still select a partial
template. Delete keeps its existing card and clears bindings through its existing path.

## Things you should know

An unrelated number-key change while a shortcut card waits also refuses the entire write. The agent can list the current
assignments and retry; neither the template nor any binding is partly written. This is the plan's whole-map
precondition.

A shortcut-changing write uses a new approval kind. An older UI cannot approve a move it cannot show; while that card
waits, its approval-list decode fails and it keeps showing its last successfully read list. This is the accepted
older-UI behavior from the plan. The upgrade path from v0.23.0 onward remains supported.

The first PR's independent review found that existing Rust constructors, including test fixtures, needed default
shortcut fields for that PR to compile by itself. Those compatibility edits are in the first PR, and its isolated
all-targets Clippy check passed.

## Open questions and possible follow-ups

None identified.

## PRs

- [#2273 — approval and atomic shortcut writes](https://github.com/scode/farhelm/pull/2273/changes) (draft)
- [#2274 — agent commands, listing, and docs](https://github.com/scode/farhelm/pull/2274/changes) (draft, based on
  #2273)

## Checks run, reused, and skipped

Each recorded test ran on an uncommitted source snapshot based on `8c200d2c`. The recorder captured each snapshot and
its selected command:

- `daa9e3a6-061a-44b3-9535-f7eedd02d595`, source fingerprint `02d0ad8231d3`:
  `cargo nextest run -p farhelm-proto -p farhelm-helm -p farhelm-ui --lib -E 'test(template) | test(shortcut) | test(approval) | test(preferences)'`
  — 131 passed.
- `9fc8b784-48a3-406a-9b70-345a88ac9bb4`, source fingerprint `b2edfebdfa00`:
  `cargo nextest run -p farhelm --test agent_cli -E 'test(template)'` — 4 passed.
- `a4036fee-cfbb-4033-87af-7fafc4dd2644`, source fingerprint `e13b855422d5`:
  `cargo nextest run -p farhelm-helm -p farhelm --lib --bin farhelm -E 'test(shortcut_edits_obey) | test(agent_instructions)'`
  — 11 passed.

All three records have complete source capture and test reports; their other test names were selection exclusions, not
runtime skips. A first all-targets Clippy attempt found one missing supervisor test initializer; after the fix,
all-targets Clippy passed on the isolated first PR and on the combined stack. Shipped CLI binary Clippy, desktop UI
compilation, Rust formatting, changed-Markdown dprint, changelog format, test-sleep source check, and diff whitespace
check passed. The docs website build passed and found no broken internal links.

The recorded CLI selection was run before splitting the commits; the restored CLI implementation is byte-for-byte the
tested version, with only its prerequisite constructor defaults moved into the first PR. The docs build was before one
plain-prose eligibility sentence was added; it adds no link, frontmatter or executable example, so its link and build
coverage still applies. The library selection predates the final already-marked eligibility case; the later focused run
covers that case, and combined-stack Clippy covers the final source.

Browser end-to-end tests were skipped because the approval change adds rows through the existing card renderer and does
not change shared card layout. The full Rust battery, desktop runtime, installer and provisioning suites were skipped
because the focused tests and compile checks cover the changed paths without a concrete remaining risk those suites
would answer. CI is on demand and was not started.

## Review gate

Each PR received the plan's single fresh-context `gpt-6.1-sol` high-effort review. PR 1's one finding, isolated
compilation, was fixed and checked; PR 2 had no findings. Both commit/PR messages passed cold reads before use.

### Landing

Landed on its own in round 95, as #2273 and #2274, merged bottom-up and unchanged from what the report describes.

#### What landed since the stack was based, and whether it interacted

Only plan-queue bookkeeping reached main between this stack's base and its landing, and no other plan landed in this
round; the landing rebase was clean. The base already held the agent wrappers (#2271, #2272), and the landing monitor
and a fresh-context reviewer both checked the stack against them, since agent wrappers made a template write that leaves
a wrapper prefix in the template always show a card with Allow or Deny only. The new shortcut-changing card follows that
rule everywhere the existing template card does: the card still appears on hosts that run commands without asking,
Always allow is still refused, and the card shows the prefix. A shortcut-only edit of a template that already carries a
wrapper still shows that card, because the card always carries the whole resulting template, so an agent cannot bind ⚡
or a number key to a wrapper without a visible Allow. Neither review found an interaction with the quick-launch key
sequences and partial templates, naming after a quick launch, or the approval cards' placement.

#### Upgrade

#2273 raises the protocol from 59 to 60. Without the bump, an older helm or supervisor would ignore the new shortcut
request and write the template without the shortcut change; with it, mismatched versions refuse to talk instead. None of
these versions has shipped (v0.29.0 carries protocol 51), so the next release already requires remote hosts to be
updated. The helm database schema is unchanged; the stack writes the existing shortcut settings.

#### Fixes made during the landing

None.

#### Possible follow-ups

- **An open window can undo an approved shortcut move.** After you approve a card that moves ⚡ or a number key, open
  windows keep showing the old holder in the Templates dialog, the ⚡ hover text and the quick switcher until a quick
  launch or a reload. Worse, the next number-key change made in that window writes back its whole old key map, which
  silently undoes the agent's approved move and drops any key the agent added; this can happen in the very window that
  approved the card. Clicking ⚡ on the template that window still shows as marked likewise clears the mark from the
  template the agent moved it to. Launches themselves are correct, because they read the current assignments. SPEC.md
  already accepts that two windows editing the key map at once may lose one edit, but not this. Refreshing the shortcut
  settings when the Templates dialog opens, or when the helm announces a change, would fix it. The report does not
  mention it.
- An agent's ordinary edit (changing only its host or session name, say) of a template that holds ⚡ but no longer
  chooses an agent or a command is refused, even without any shortcut flag, which narrows plain template edits that
  worked before this plan. The Templates dialog can produce such a template; SPEC.md's new wording covers the refusal
  and a test pins it, but the refusal message could point the agent at clearing the mark.
- A ⚡ or number-key assignment that names a template that does not exist yet attaches to a template of that name that
  an agent creates later, and the card does not say so. This predates the plan.
- A protocol-level test checks that the new card kind needs an explicit Allow, but no helm test runs a shortcut-only
  edit of a wrapper-bearing template on a host that runs commands without asking.

#### Checks

- Run now, on the rebased stack (the landed diffs are identical to it): `cargo fmt --all -- --check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo check -p farhelm-ui --features desktop`; through the test recorder with pinned nextest and tmux, four slots and
  no retries, the whole protocol, helm, supervisor and UI libraries, the `farhelm` binary's tests and the agent
  command-line test file: 3271 passed and one failed, run `e5bf0834-f29d-49e6-9264-e3bdd186ea5d`. The failure was a UI
  timing test the stack does not touch (`a hung save releases waiters at their deadline`): its 30 ms wait measured
  slightly short in the full run. Run alone 40 times it passed every time, run `b071b4cd-8d09-48d4-bffb-51a7a9104e57`.
  It is one observation, kept in the retained run and not yet recorded as a flake; an early timer is not explained by
  load alone, so a recurrence should be logged. The UI JavaScript harness, run directly: 294 passed. Then fresh
  `cargo build` and web builds and `npx playwright test approval-layout.spec.ts templates.spec.ts quick-launch.spec.ts`
  on both engines, one worker, no retries: 182 passed, run `31b65753-b6ba-4187-9a30-0dc8d26fa797`. The executor had
  skipped browser tests.
- Reused: the executor's website build, changelog, test-sleep and formatting checks on the delivered source.
- Skipped: everything else the report skipped, for the reasons it gives.

Nothing in the executor's report became untrue at landing.
