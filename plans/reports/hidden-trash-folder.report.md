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
