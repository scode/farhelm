# Archive wake regression test covers Delete but leaves Replace unguarded

Reviewed commit: 7e90bf0f704e68e400f5d042a7bef5b943d055c2

## TLDR

The new test would still pass if a future edit broke automatic Trash refresh after Replace removes its source.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/sessions_tests.rs:1717`.

SPEC_impl.md:3869–3872 requires a fleet wake for both Delete and Replace even when the source row is already gone.
Ordinary Delete and finish_replacement have separate publication sites at sessions.rs:4138–4141 and :5047–5054. The new
delete_session_happy_path_returns_200_with_the_archive_fact gates a DELETE reply, forgets the row, and asserts the exact
revision difference at sessions_tests.rs:1720–1810. The existing Replace source-cleanup test at :3683ff checks a timely
warning, not a moved checkout or a cache-forget-independent wake. Carry the pre-forgotten-source test premise through
the Replace entry point or unify the publication path and cover both entry points. Bucket: other (test oracle gap).
Confidence: confirmed by source assertions. Possible coverage: timely Replace warning and ordinary Delete wake are
adjacent but independent behaviors.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `notices/f3`.
