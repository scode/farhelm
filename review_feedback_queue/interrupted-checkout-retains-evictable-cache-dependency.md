# Cache eviction can invalidate an interrupted checkout reused for new work

Reviewed commit: 247f4760b3477fee340b5bd010b14878992fb518

## TLDR

After a hard interruption, a partially cloned repository can still depend on the cache. If the user resumes work there,
later cache cleanup can make that repository incomplete.

## Details

Suggested bucket: `highest`. Confidence: likely; source establishes the interruption window and missing dependency
tracking, but no runtime reproduction of hard interruption followed by user reuse and eviction was performed. Corruption
from an orphaned live Git child racing eviction is also unverified.. Source: dedicated `gpt-6-astra` high adversarial
review. Proposed drop: None. Preserve for independent assessment, particularly the unverified user-work consequence and
orphan-child variant..

At crates/farhelm-supervisor/src/launch.rs:1535–1548, independence depends on Git completing --dissociate.
repo_cache.rs:66–79 subsequently permits eviction whenever the shim's lock is free and its timestamp is old; it does not
inspect retained preparations or borrowing repositories. Git writes refs before dissociation and removes the alternates
file only after repacking, so SIGKILL during that interval can leave a usable repository dependent on cached objects.
Ordinary Git error/signal cleanup usually removes the unfinished repository; this candidate specifically requires an
interruption that bypasses cleanup. See
[Git v2.54 clone implementation](https://github.com/git/git/blob/v2.54.0/builtin/clone.c), lines 389–420, 843–858 and
1602–1628. Farhelm explicitly suggests opening an ordinary session in incomplete contents (launch.rs:1383–1401), and
borrowers bypass the original preparation's Ready requirement (service/core.rs:9987–10002). After the user restores the
working tree and creates work, eviction can remove objects its history still requires. A related lifetime gap exists if
only the shim dies: CLOEXEC closes cache protection while a Git child can remain alive. Successful, fully dissociated
checkouts are unaffected. Consider retaining cache dependencies across interrupted preparations or explicitly resolving
them before reuse/eviction.

Possible cover: SPEC.md:675–677 accepts incomplete preparation and refuses repetition, but does not explicitly accept
later invalidation of work created in that directory. Current SPEC.md:1040–1050 concerns particular cleanup/resume
failures, not repository corruption. No matching Planned item, BUGS entry, original queue item or searched ledger
decision found.

Status at frozen endpoint: Persists structurally at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703: repo_cache.rs is
unchanged; launch.rs:1626–1642 still relies on successful dissociation, and service/core.rs:10195–10209 still permits
borrowers of incomplete contents. No fixing commit found.

Review coverage: full commit diff read: true. No commit diff unread. Unchanged surrounding modules were read
selectively, not repository-wide. The 13,089-line TRIAGE_OUTCOMES.md was searched comprehensively for relevant terms but
not read in full; unrelated queue finding bodies were not opened. An initial combined policy output was truncated;
relied-upon missing documents/passages were subsequently reopened in bounded output. Highest-priority proposed drops and
refuted highest candidates still require the parent's independent drop review; no helper agents were spawned.

Runtime evidence: None, as requested by default. No checkout files or scratch files were created, modified or deleted.
No fetch, rebase or range extension. Changed tests/fixtures were assessed against the supplied checklist by inspection;
interruption/data-loss consequences remain unverified at runtime.
