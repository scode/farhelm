# A successful native download lacks directory crash durability

Reviewed commit: 550e7e592a6489cfc475b71c678131de4d9f67e7

## TLDR

A power loss immediately after a successful desktop download can lose the saved file's directory entry.

## Details

Suggested bucket: `highest`. Confidence: likely conditional durability gap; filesystem-specific persistence is
unverified, and no power-loss reproduction was performed. Source: dedicated `gpt-6.1-sol` high review, retained by a
fresh independent Sol high drop auditor. Proposed drop: rejected as ambiguous, for acceptance adjudication rather than
as a proven runtime incident.

crates/farhelm-helm/src/downloads.rs:181–192,225–239 reports success after no-clobber publication.
crates/farhelm-supervisor/src/files.rs:649–667 syncs file content, creates the destination hard link, removes staging,
and returns without syncing the destination directory. The conditional consequence is loss of the downloaded copy after
a machine crash despite successful publication being reported. Assess whether the Downloads contract promises this
durability; if it does, establish a destination-directory barrier before reporting success.

Possible cover: historical files.rs:134–137 describes the helper's best-effort tier for attachments. Current
SPEC.md:1565–1567 and TRIAGE_OUTCOMES.md:8484–8496, published-uploads-lack-directory-fsync.md, explicitly accept
attachment/upload power-loss uncertainty. Native Downloads has a separate contract at SPEC_impl.md:1702–1707. No
explicit acceptance for downloaded copies was found; the scope ambiguity is retained for triage.

Status at frozen endpoint: persists at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703. downloads.rs is unchanged and
files.rs:649–672 still publishes without a destination-directory sync. No fixing commit identified.

Review coverage: the original reviewer read the full commit diff; the independent reviewer read publication and
acceptance paths. No runtime or power-loss experiment was run.
