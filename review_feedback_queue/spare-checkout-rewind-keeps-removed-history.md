# Spare checkout can retain history removed upstream

Reviewed commit: 343faa698aae5bf2907aa1b84ec6bc9b5a08b667

## TLDR

After the repository's default branch is rewound, a managed-checkout launch can report successful preparation while
starting from commits upstream removed. The spare-update recipe fast-forwards in a way that treats an ahead checkout as
already up to date.

## Details

Suggested bucket: **high**. Confidence: **confirmed static sequence conditional on an upstream rewind**; no runtime
frequency, security consequence or loss of user work is established.

The reviewed plan at `plans/queue/spare-launch-checkout.md:90–114` checks branch existence/default-name agreement, moves
the prepared contents, then runs `git merge --ff-only`. If the spare has B and upstream rewinds to ancestor A, merging A
succeeds without moving local HEAD or the worktree back from B. The plan's fallback and never-worse wording
(`86–89,309–312`) does not explicitly require checking this history relationship before moving.

Recording-time reconciliation found the later implementation at
`47ccf5fbfcf323094bd2133a7e2b6771797bc98e:crates/farhelm-supervisor/src/spare_take.rs:87–166,351–414` retains the same
sequence without an ancestry/rewind check. These later commits are outside the trailing-review range; only this focused
correction/coverage check was performed. Compare the prepared HEAD with the fetched tracking commit before moving, and
handle rewinds/divergence through an explicitly correct strategy.

Possible cover: later `SPEC.md:1041–1052` accepts timestamps, retained Git objects, hook/filter differences and visible
failure after moving. It does not explicitly accept a successful checkout of ahead-of-upstream removed history.
Historical divergent-merge failure is partly covered by the post-move-failure policy and is not the retained remainder.
No exact other acceptance or duplicate was found. Proposed drop: none.

Provenance: trailing-review run `a7b93df6-ca17-4bff-8608-f443575ae5e6`, refined `g027/f2`. The separate background
askpass omission `g027/f1` was repaired by the later implementation and is not retained as an open item.
