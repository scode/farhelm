# Refuse updates that cannot read a host’s database

## What this was about

Trying a development build can upgrade a remote host’s saved data. Updating that host back from the installed release
then went ahead because development builds rank below releases, even when the release could not open that database. The
replacement supervisor restarted repeatedly and the host stayed unreachable until the newer build was put back.

The maintainer chose to keep devbuild’s deployment behavior and make Update refuse before changing the host when its
database needs a newer build. Execution originally blocked because the reviewed fix exceeded the agreed size limit. The
maintainer accepted the built fix, including hosts using a different protocol version, and waived that limit.

## Things you should know

Update now compares the database version reported by the host with the version its replacement supervisor can open. It
checks both when preparing the update and again before carrying it out. The refusal explains the mismatch and says to
update from a build at least as new as the one that last ran there. The check also works when the host and helm use
different protocol versions.

Protection requires a helm with this fix and a supervisor that reports its database version. Older helms ignore the new
information; an older supervisor that does not report it keeps the previous behavior. A development build made before
this fix can therefore still leave a host unprotected. The existing warning about development builds sharing remote
hosts still applies.

This does not change devbuild’s choice of supervisor, migrate any database, or change normal build-version ordering. Two
branches assigning different meanings to the same database version remain outside this plan. The devbuild instructions
now carry the update-back caveat.

## Open questions and possible follow-ups

No maintainer decision remains. The originally accepted fix was 288 changed lines of code, tests and documentation.
Final review corrections bring that to 329, using the same design: clearer failure text and compatibility caveats,
constructor documentation, and stronger tests. The older-peer limitation and the same-number migration collision remain
as described above.

## The PRs

- [#2649](https://github.com/scode/farhelm/pull/2649/changes): refuse Update before installation when the host reports a
  database newer than the replacement can read.

The PR remains a draft for the landing monitor. It has not been merged by this executor.

## Checks run, reused and skipped

Final recorder run `0623921f-3c78-4a1a-b8e2-782a7320651b` passed four selected cases, with 2,749 filtered out, four
nextest slots and zero retries. Its complete source record is the dirty tree based on `6b0d76cf` with the final review
corrections; the later ledger and message edits do not change tested behavior. Pinned nextest and tmux identities
matched, output and the test report were complete, and no selected case reported a runtime skip. The child command was:

```sh
cargo nextest run -p farhelm-proto -p farhelm-helm -p farhelm-supervisor --lib -E 'test(=provisioning::tests::update_refuses_a_host_whose_database_schema_is_newer) | test(=provisioning::tests::update_refuses_a_host_running_a_newer_farhelm) | test(=io::tests::handshake_refuses_protocol_mismatch) | test(=tests::new_hello_json_decodes_under_a_legacy_pre_host_identity_decoder)'
```

This covers higher-schema refusal and equal/missing-schema acceptance, revalidation at confirmation on both protocol
paths, the existing build-version refusal, schema retention through a protocol mismatch, and legacy decoding with the
literal serialized key pinned.

The real supervisor greeting test passed in recorder run `6a011d31-6d7e-4233-83c6-0f54ccf92bd3`: one executed case
checks the reported schema through the production connection path across a supervisor restart. Its complete recorded
source was based on `6b0d76cf`. Later changes only improve refusal text, expand the refusal test, pin a serialized key,
and correct documentation; they leave that producer behavior unchanged, so this result is reused.

The pre-correction focused run `204dfa7c-b226-4487-8607-6c995f234de1` passed four cases. An additional selector named a
helper rather than a runnable test and supplied no producer coverage; the separate producer run above corrected that
gap. The final corrected run supersedes those four results for current refusal and wire behavior. Prior run
`2b780c27-fd76-4892-8055-2fdcd143f36e` passed 432 selected protocol, provisioning and connection tests on a complete
recorded dirty tree based on `ba942de5`, with pinned tmux matched. That is supporting evidence from before the rebase,
rather than a fresh whole-suite verdict.

Final `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`, touched-Markdown `dprint check`,
changelog format validation, and the isolated test-delay checker passed. The instruction checker passed with zero errors
and one existing warning about an unrelated triage citation. The delay checker inspected 314 delays with no missing
rationale.

The whole Rust battery, browser tests, installer and real-host deployment were skipped. The changed behavior is in host
Update admission and the supervisor’s greeting; focused transport and provisioning tests cover those boundaries, and
compilation checks the combined fixtures. The tests use isolated supervisors and injected provisioning operations. No
live installation was updated or otherwise changed.

## Review gate outcome

The required single Opus 5.5 reviewer at high effort passed the final change. Its initial review found misleading
refusal attribution, an incomplete compatibility caveat and test gaps. Those were corrected, and the same reviewer
verified all six findings were resolved with no new issues. A separate scope reassessment at the executor’s own model
found no unnecessary complexity.

The native resume audit and the complete upstream-change audit found no conflicting decision or design change. The only
rebase conflict was in the feedback index; both plans’ removals were preserved. The ledger now records the maintainer’s
size waiver and completion. Commit and PR wording passed a fresh cold read.
