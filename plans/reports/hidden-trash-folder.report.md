# Hidden managed-checkout trash

## What this was about

Deleting a managed checkout's last session preserved its work in a visible `farhelm-archived-working-copies` directory
beside the user's checkouts. Trash now lives in hidden `.farhelm-trash`, matching the spare checkouts' hidden container.
Existing trash moves automatically before the next archive, recovery, list or permanent delete on that checkout root.

## Things you should know

A session started inside the old trash keeps its recorded working-directory path. As accepted during planning,
restarting that session can fail after migration; the retained session still prevents permanent deletion of its
checkout. No mechanism delays migration while such sessions exist.

Migration preserves the whole directory by renaming it without replacement. A checkout root that becomes reachable after
startup migrates before missing-archive cleanup can forget its records. Both names remain reserved. No registry schema
or archive destination records change.

If both directories exist after an out-of-band change, the old contents stay on disk but are not shown or cleaned by
Farhelm. Listing can forget their trash records because only the new directory is used. Recover wanted work manually
from the old directory; there is no automatic merge. This is not the normal upgrade path.

An active managed checkout overlapping the old directory prevents migration. Trash listing and permanent deletion report
the refusal and retain the archive records. Migration can succeed on retry once Farhelm no longer records a managed
checkout at, inside or around that old directory. If deleting a last session cannot archive its checkout because
migration is refused, the session is still deleted, the checkout stays in place and the not-archived notice names it.
Other migration I/O failures can be retried once their filesystem problem is resolved.

The GitHub-checkout guide and the website's Stop, restart and resume page describe the hidden directory and automatic
move. The staged screenshot path was updated; published screenshots and historical release material were left alone. The
plan's TODO entry was removed and a changed changelog fragment includes the retained-session caveat.

## Open questions and possible follow-ups

None identified.

## PRs

- [#2744](https://github.com/scode/farhelm/pull/2744/changes): hidden trash with automatic migration and
  retained-session protection.

## Checks run, reused and skipped

The final focused Rust command selected working-copy allocation/archive/recovery, trash listing and deletion, repository
discovery, teardown and core checkout/archive cases:

```sh
cargo nextest run -p farhelm-supervisor --lib -E 'test(/working_copies::|checkout_trash::|repository_discovery::|service::teardown::|service::core::tests::.*(checkout|archiv|allocat|last_reference)/)'
```

It ran through the recorder with pinned nextest and tmux, four slots and no retries. Run
`beed4aa2-782f-4deb-a541-5170c41abd25` passed all 149 selected tests, with 1,185 selected out and no runtime `SKIPPED`
messages. This covers the ten new migration/retention tests, existing overlap/identity guards and the core allocation
callback's legacy-name reservation.

The preceding run `8c00379a-44fb-42f3-87c6-637033e7bcb2` passed 148 of 149 and failed an old test's expected directory
ordering: the hidden name sorts before `bar`. Correcting that expectation preserved its refusal, unchanged ownership and
untouched-directory assertions. The exact retry `73a832dd-4608-43e0-8019-ed8dd9716f76` passed, then the final 149-case
command passed. The initial focused migration run `0c0e9ee6-b3ef-460c-8fee-4b256aa86883` also passed all 21 selected
tests. Failed evidence was retained; this was a deterministic same-change test adjustment, not a latent flake.

Reused the final 149-case result after removing an unnecessary clone from one test's expected-ID assertion: both sides
still compare the same strings, and no product code changed. Its complete source fingerprint was
`49b5da755792b997a63e027afe47c5cb536a109a258c3fcc54f7b22c051a1d76`. The corrected assertion is recompiled by Clippy. The
test-delay, fragment and instruction checks were also reused after that assertion-only edit: it changed no delay calls,
Markdown, fragments or instruction targets. Formatting was checked again on the final source.

`cargo check -p farhelm-supervisor --tests`, `cargo fmt --all -- --check`, changed Markdown `dprint check`, the isolated
parser-backed test-delay check (322 delays, zero missing rationales), changelog format validation and the
agent-instruction link/size check passed. The instruction check retained two pre-existing citation warnings. The initial
compile check caught outdated test helper arguments and an unsupported equality assertion, corrected before the passing
check. The first Clippy attempt caught the unnecessary clone described above.

`cargo clippy -p farhelm-supervisor -p farhelm-helm --all-targets -- -D warnings`, `cargo build` and the UI's release
web build passed on the final source. No application build was reused from another plan.

The recorded browser command
`npx playwright test tests/github-checkouts.spec.ts tests/trash-dialog.spec.ts tests/modal-layout.spec.ts tests/popover-width-movement.spec.ts`
passed all 134 cases (67 Chromium, 67 WebKit), with three workers and owned stacks, pinned tmux and no retries. Run
`62e485c3-9314-44f7-86af-75e23d90a1de` has complete reports, zero failures, skips or unstarted cases, and complete
source fingerprint `6efa99cd5e7f6273dad14d9ea8e23bbcb0c1da08b927ea3cbc2c3330dfe41dd4`. Its recorded source diff matches
the published PR's commit `858590dc` byte for byte. These cases include real on-disk archive creation, listing,
single/global deletion and retained checkout use, alongside trash and layout fixtures.

The full Rust workspace, unrelated browser specs, installer/provisioning and native desktop runtime were skipped:
targeted coverage exercises the changed behavior and no installation or platform-specific code changed. The website edit
changes ordinary prose, with no heading, link or generated artifact changes; no website build or preview was needed. The
configured dprint plugins do not handle MDX; that page was inspected directly. Screenshots were not recaptured or
published because this task updates a storage path rather than their design.

## Review gate's outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort reviewed correctness, design, idiomatic code and
the complete test-authoring rules. Findings were resolved: an old title-based reservation fixture needed the legacy
name, two comments needed the current path and migration contract, and a proposed row-snapshot optimization had an
allocation race. The final implementation keeps the fresh overlap check and rename under the allocation database lock,
then releases it before individual archive opens and sizing. The same reviewer inspected that replacement, the sorted
expectation and the clone removal, and reported no remaining findings. The cold commit/PR wording reader also completed.

### Landing

Landed as built. Deleting a managed checkout's last session keeps its work in Trash, which used to be a visible
`farhelm-archived-working-copies` folder next to the user's checkouts. Trash is now the hidden `.farhelm-trash`, like
the spare checkouts' hidden folder, and an existing old Trash folder moves there automatically the next time Farhelm
archives, recovers, lists or permanently deletes on that checkout root. The GitHub-checkout guide and the docs page
"Stop, restart and resume" describe the new location.

- #2744: hide checkout Trash and move the old folder automatically.

#### What landed since the stack was based, and whether it interacted

Since the plan was based, main gained four plans from the previous landing round and review-queue triage records. Two of
those plans change code next to this one. "Restart with" now asks the supervisor whether a session's folder is a managed
checkout before trusting it automatically; a folder in Trash, under the old or the new name, was never a managed
checkout and still is not, so this plan changes no trust decision. The other changed how a named restart treats the
saved conversation, which shares nothing with Trash. A fresh-context reviewer and I read this plan against everything
main gained and found nothing that needs fixing.

#### Fixes made at landing

None.

#### Things you should know

- While Trash is being listed, the one-time move of the old folder (a check that no live checkout overlaps it, then the
  rename) runs while the supervisor holds its database lock; the lock is released before each trashed checkout is opened
  and measured. The plan's reviewer chose this to avoid a race with a checkout being created at the same moment. On a
  network disk that hangs during that move, other supervisor work that needs the database would wait with it. It happens
  once per checkout root; I would accept it unless you use checkout roots on network disks, in which case it is a
  candidate follow-up.
- Both folder names stay reserved: neither can be the name of a new checkout. But the check that stops a new checkout
  being placed inside a Trash folder now knows only `.farhelm-trash`, not an old `farhelm-archived-working-copies`
  folder that has not moved yet. Reaching that takes choosing a checkout root inside another root's old Trash. While
  that checkout exists, the outer root's old Trash cannot move: listing and permanent deletion there report the refusal
  and keep the Trash records, and deleting a last session there leaves its checkout in place, with a notice, instead of
  moving it to Trash. No work is deleted. It is an unlikely setup; a follow-up only if you think anyone would do it.
- The queued review finding "hidden-trash-overlap-migration-result-source-gap" asked what happens when the move is
  refused; this plan settles it (the refusal is reported and the Trash records are kept), so it can be closed when you
  triage it.

#### Checks

This round landed only this plan, on main after the previous round's four plans:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, the check that every test delay states its reason, `dprint check`
  across the repository, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (two
  warnings, both about citations main already had), the docs website build (for the edited page), the workspace debug
  build and the UI's release web build: all passed.
- The four browser specs the plan changed, `npx playwright test` in `e2e/` with `tests/github-checkouts.spec.ts`,
  `tests/modal-layout.spec.ts`, `tests/popover-width-movement.spec.ts` and `tests/trash-dialog.spec.ts`, on Chromium and
  WebKit, three workers each with its own helm and supervisors, no retries, run `0ad0d661-cb34-477c-805a-2dbdfa795f82`:
  all 134 passed. They include creating a real Trash on disk, listing it and deleting from it.
- The whole Rust workspace, `cargo nextest run --workspace --exclude farhelm-desktop`, with the pinned test runner and
  tmux, four tests at a time, no retries, run `c90cdc02-009a-43b7-abed-45744177a404`: all 4466 passed; 40 more were
  skipped by design (helper entries that only run as child processes of other tests, and tests that need a real
  signed-in Claude or Codex); the retained output has no in-test `SKIPPED` message.
- Reused: none. Skipped: the full browser suite (the changed specs cover the Trash surfaces), the desktop app checks (no
  desktop code changed) and a real Mac (not runnable here).

The report skipped the whole Rust workspace and the website build; this landing ran both. Nothing in the executor's
report became untrue at landing.
