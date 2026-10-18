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
