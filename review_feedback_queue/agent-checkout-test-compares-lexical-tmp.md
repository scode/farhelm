# The new CLI checkout test compares canonical paths with lexical /tmp paths

Reviewed commit: c8de519d6351af429a9c2ff519bc54774a97b8ed

## TLDR

The new checkout test fails on macOS despite successful product behavior.

## Details

Suggested bucket: `other`. Confidence: confirmed by code inspection; not executed on macOS. Source: dedicated
`gpt-6.1-sol` high adversarial review. Proposed drop: none.

crates/farhelm/tests/e2e/github_checkouts.rs:88–91 and 177–180 compare session.cwd with root_a.path().join(...). The
fixture uses farhelm_teststate::tempdir (382), which constructs a lexical path beneath /tmp without canonicalizing it
(crates/farhelm-teststate/src/lib.rs:65, 640–684). Managed admission records a canonical destination. On macOS, /tmp
resolves to /private/tmp, so these assertions disagree. Existing tests in the same file already canonicalize their
expected roots (1082–1088 and 1123–1134). Canonicalize the fixture root before constructing both expectations.

Possible cover: TRIAGE_OUTCOMES.md heading checkout-recovery-compares-canonical-roots-unresolved-fixture.md records a
completed fix for older assertions. It does not cover these newly introduced assertions.

Current-main status: Persists at pinned main, at the same test lines. git diff between the reviewed commit and pinned
main for this file is empty; no fixing commit.

Review coverage: full commit diff and relevant code paths read. Runtime behavior and harmful reliance were not
reproduced; the note preserves the reviewer's stated unverified premises.
