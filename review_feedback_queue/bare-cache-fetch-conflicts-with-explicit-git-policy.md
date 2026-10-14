# Explicit-only bare repository policy blocks every fresh checkout

Reviewed commit: 247f4760b3477fee340b5bd010b14878992fb518

## TLDR

Hosts using Git’s safe.bareRepository=explicit setting cannot launch sessions in fresh GitHub checkouts.

## Details

Suggested bucket: `high`. Confidence: confirmed. Source: dedicated `gpt-6.1-sol` high adversarial review. Proposed drop:
none.

crates/farhelm-supervisor/src/launch.rs:1515–1533 selects the new bare cache with -C. Git’s explicit policy requires
--git-dir or GIT_DIR instead. Cache initialization succeeds, but fetch refuses before contacting GitHub; :1429–1430
records a permanent clone-stage failure, preventing clone, hook and agent execution. Ordinary clones worked before this
commit. A recorder-backed Git 2.43.0 probe returned 128 with the explicit-policy diagnostic; the equivalent --git-dir
fetch returned 0. Select the supervisor-owned cache explicitly without disabling the user’s security policy. See
[Git’s policy documentation](https://git-scm.com/docs/git-config#Documentation/git-config.txt-safebareRepository).

Possible cover: SPEC_impl.md:1565–1568 specifies the current command shape, and SPEC.md:670 accepts failing safely on
cache errors. Neither explicitly accepts incompatibility with this supported Git security setting; triage should assess
that distinction.

Status at frozen endpoint: Persists at pinned main 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703, launch.rs:1599–1620. No
fixing commit found.

Review coverage: full commit diff read: true. No commit-diff content remains unread. Initial truncated aggregate output
was recovered through smaller reads. Unrelated bodies of the 13,089-line triage ledger, unrelated specification
sections, and unrelated queue files were not read exhaustively. Deep unchanged Stop/Delete process-reaping internals
were not independently audited. No nested reviewers were launched; proposed highest drops remain pending independent
review.

Runtime evidence: Two recorder-backed diagnostic probes using Git 2.43.0, one Git child at a time, --tmux none, with
child-specific environments and all artifacts under the permitted scratch directory. Run
6cd3d028-0987-4ca3-b05f-dcf3254a8300 failed during inherited-GIT_COMMON_DIR fixture initialization and is retained. Run
14039aa4-b005-4edc-81cc-ce3d3b096da2 retained that boundary, observed the foreign tag unchanged, reproduced safe-bare
fetch refusal (128), and observed explicit --git-dir fetch success (0). These were top-level diagnostic probes invoked
through deno test, which reported zero registered tests; they were not Rust or product-suite runs. Deno formatting and
type checks passed. No checkout files were created, modified or deleted by this reviewer.
