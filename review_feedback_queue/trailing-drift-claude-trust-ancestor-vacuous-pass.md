# Claude trust drift case can pass vacuously when the copied config trusts an ancestor folder

Reviewed commit: ba8dbf0e98a9f2cdff1f7ca533d5ee6baa529d38

## TLDR

The live check meant to catch a Claude release that stops honouring the trust Farhelm records can pass even when that is
broken. If the maintainer's own Claude configuration already trusts /tmp or / (for example after once accepting Claude's
trust question there), Claude skips its trust question for every folder below it, including the test's scratch folder.
The test then sees Claude open without asking, finds Farhelm's entry in the copy, and reports a pass without ever
showing that Farhelm's entry is what made Claude skip the question. Whether the maintainer's configuration actually
trusts such a folder is not known.

## Details

Concerns 547d11f042 (`check_claude_trust` and the Claude arm of `workspace_trust::check_launch`) and 21f0fcadd8 (routes
the same check through `TrustEvidence::ClaudeConfig`). Main reference
`crates/farhelm/tests/e2e/real_agent_capture/drift/private_config.rs:526-547` (`PrivateConfig::check_claude_trust`; the
guard is the `before ... !claude_project_is_trusted(bytes, key)` ensure), with the mirror copies built at
`private_config.rs:465-524` (`claude_trust_mirrors`) from the user's real config. Caller: `check_launch` in
`workspace_trust.rs` (the `TrustEvidence::ClaudeConfig` arm). The scratch workspace root is `farhelm_teststate`'s
`TMP_ROOT` "/tmp" with prefix "fh-it." and inner dir "d" (`crates/farhelm-teststate/src/lib.rs:70`, `:78`, `:769`), so
the workspace is `/tmp/fh-it.<random>/d`.

The case's evidence for Claude is (1) Claude reaches its composer without a trust question (`FailOnPrompt` in
`Terminal::ready` and `check_startup_through_composer`) and (2) `check_claude_trust` finds
`projects[<canonical work>].hasTrustDialogAccepted == true` in a private copy, and that the same exact key was not
already true before. The private copies are byte copies of the user's real `~/.claude.json` and related files
(`build_with_copies`), so every other `projects` entry, including ancestors of the workspace, carries over.
SPEC_impl.md's Claude trust paragraph ("Trust skips home and every parent of home, including the filesystem root,
because Claude inherits trust in subdirectories") establishes that Claude extends a folder's trust to its descendants.

With `projects["/tmp"]` (or "/") trusted in the copy, Claude would not ask in `/tmp/fh-it.<random>/d` even if it ignored
Farhelm's key (say a vendor renamed the field or moved trust storage, exactly the drift this category exists to detect),
so (1) holds vacuously and (2) only proves Farhelm wrote a key. The docstring of `check_claude_trust` claims it shows
trust came from Farhelm's write "instead of relying on a trust decision the vendor already knew"; an inherited ancestor
decision is such a decision and is not checked.

Verify by reading `check_claude_trust` (exact-key comparison only) and `claude_project_is_trusted` (single key lookup).
Fix: before launch (or in `check_claude_trust`), fail or report could-not-run when any ancestor of the canonical
workspace is trusted in the selected copy's `before` bytes; or remove ancestor trust entries from the private copies
when building them, which keeps the user's real file untouched.

Bucket: other (the opt-in live drift test can give a false pass; no user data or security impact by itself). Not
highest: the consequence is a missed vendor-drift signal in maintainer tooling, and a regression it misses would surface
to users as Claude asking a trust question, not as a security loss.

Confidence: likely. Source establishes the vacuous pass whenever an ancestor is trusted in the copied config; the
unverified premise is whether the maintainer's real config trusts /tmp or / (not observed).

Possible cover: none known.

Provenance: Trailing code review of f6d8d8bb..ba8dbf0e (2026-11-25), reviewer group g06 record f2; claude-opus-5-5
reviewer, source analysis only, nothing executed.
