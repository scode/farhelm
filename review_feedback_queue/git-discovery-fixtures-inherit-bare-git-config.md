# Fixture isolation misses bare GIT_CONFIG

Reviewed commit: 22415ecbc85b970d66acfb51e1d0950da77f3366

## TLDR

Running repository-discovery tests can still overwrite configuration belonging to another repository or the user.

## Details

Suggested bucket: `highest`. Confidence: confirmed by code and installed Git documentation; no destructive reproduction
run. Source: dedicated `gpt-6.1-sol` high adversarial review. Proposed drop: none.

Incomplete fix at crates/farhelm-supervisor/src/repository_discovery.rs:993–996: filtering names beginning GIT_CONFIG_
leaves GIT_CONFIG inherited. The fixture at :571–574 executes git config include.path <temporary-file> without --local
or --file. Git's installed git-config documentation explicitly defines GIT_CONFIG as an implicit --file for this
command. If it names a writable caller-owned config with one include.path value, the fixture replaces that value;
without one, it adds a dangling include. Temporary-directory cleanup deletes the referenced fixture file without
restoring the foreign config. Scope is test execution, not shipped checkout preparation. Remove GIT_CONFIG explicitly or
bind fixture configuration writes to their owned repository. This path already existed in the first parent and remains
uncovered by this isolation change.

Possible cover: TRIAGE_OUTCOMES.md:7244–7258, heading git-fixture-setup-modify-delete-callers-repository.md, covers the
GIT_DIR trigger and GIT_CONFIG_* removal. Bare GIT_CONFIG is a distinct remaining redirect; the completed outcome does
not demonstrate its resolution. No matching current queue entry, Planned item, BUGS entry, or filter acceptance found.

Current-main status: Persists at pinned main 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703: repository_discovery.rs:571–574
and :986–999 are unchanged. No fixing commit.

The full commit diff and relevant failure paths were read. No runtime reproduction was run.
