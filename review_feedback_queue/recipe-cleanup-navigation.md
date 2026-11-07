# Cleanup navigation failure skips owned recipe rows

Reviewed commit: aebf561680cc851498c6619d28b1be1ed3600617

## TLDR

If navigation away from a recipe-mode browser test fails, its recipe and credential deletion never runs. Fictional rows
can then contaminate later tests using that private stack. This is maintainer-test cleanup, not loss of users'
credentials.

## Details

Suggested bucket: **other**. Confidence: confirmed conditional source bug, without runtime reproduction or a frequency
claim. Also reviewed at `d5b4a317286682edac7e3dd402f981fae8b50410`; pinned context is
`49ce8cb76e9f4c841e15122a89264c7c908171dc`.

`e2e/tests/tensorlake-recipes.spec.ts:563–569` awaits `page.goto('about:blank')` before removing recipes and deleting
owned credentials. No inner finally or all-settled boundary makes API cleanup independent of that await. `isClosed`
handles an already closed page, not navigation rejection. Successful creation at `503–509` establishes ownership.
Depending on prior progress, remaining rows include the test's mode recipe and either or both login fixtures. The
earlier reviewed revision has the same unconditional navigation boundary at `548–554`.

The observation fixtures (`helpers/evidence.ts:46–111,370–402`; `helpers/host-premise.ts:167–178`) and global setup
(`global-setup.ts:56–118`) do not independently delete these rows. This spec has no afterEach/afterAll cleanup for them.
Per-slot stacks persist until run end (`playwright.config.ts:125–145,170–230`); eventual whole-stack reclamation
(`start-stack.sh:375–447`) does not repair between-test contamination.

Attempt owned API cleanup independently of navigation success, and preserve cleanup errors without skipping the other
owned rows. The separately retained accepted-POST race is an extension to
`failed-test-response-does-not-fence-settings-write.md`; navigation rejection needs no server-acceptance or
document-survival premise. Existing route-retirement and preference-reset cleanup items have different failure
boundaries and owned resources. No exact acceptance, planned repair, ledger or filter coverage was found.

Provenance: run `4ca8e296-4f73-4429-a34b-031af1582085`, `g008/g008-l003`, narrowed from its original composite record;
`g007/g007-l002` corroborates the separate accepted-POST boundary. Proposed drop: none for this navigation route.
