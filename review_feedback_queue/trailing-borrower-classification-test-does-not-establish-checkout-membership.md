# Borrower classification test does not establish checkout membership

Reviewed commit: 85cf6b2e7715c5dce446bcc95170662c1fbc8bb8

## TLDR

This borrower classification case can pass if its second session never joins the managed checkout.

## Details

**other.** Source inspection. Main reference `crates/farhelm-supervisor/src/service/core.rs:37140`.

core.rs:37126-37146 confirms the first session owns a managed copy, then creates another session in its cwd through
create_session_without_overrides, the wrapper for the ordinary create path (:8940-8973). The assertions at :37163-37240
check terminal acceptance, absence of preparation error and reload/storage agreement. They never observe the second
session's membership. status.rs:504-525 asks for preparation_origin, which reads ownership after generation and
acceptance checks (store.rs:3573-3598), so a proper borrower and a plain nonmember yield the same negative
classification. SPEC_impl.md:3466-3471 distinguishes joining the canonical managed ancestor from owning its preparation.
Assert durable membership of the exact checkout and lack of preparation ownership before the status/reload checks. This
remains separate from f7's origin-runtime handoff. Bucket: other. Confidence: source-confirmed fixture-premise gap. No
shipped membership failure established. Possible coverage: core.rs:34698-34761 separately checks the borrowed
association on ordinary create and listing; :37710-37791 directly checks durable membership and no ownership for an
interrupted borrower, but neither proves the exact specimen in this classification fixture.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `lifecycle_unit/f16`.
