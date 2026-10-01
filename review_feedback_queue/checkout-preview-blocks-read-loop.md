# The GitHub checkout preview stalls every terminal on its host

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

While the user types a GitHub repository into the create dialog, typing in every open terminal on that host can freeze
for as long as the supervisor takes to scan the checkout folder, which on a slow network home directory or a very large
folder can be noticeable.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F5 / COR-PREVIEW-READLOOP`, tagged **possible**. Anchor and title:
`crates/farhelm-supervisor/src/service/handlers.rs:2893` — the GitHub-checkout preview runs inline on the helm
connection's read loop with a synchronous directory scan.

When a user types a GitHub repository into the create dialog, the helm asks the target supervisor for a preview: the
folder name a fresh checkout would get under the configured checkout root. Each supervisor serves its connection from
the helm with a single read loop, and that same loop carries keystrokes, resizes, detaches, upload chunks and every
request for every session on that host. Anything that loop awaits directly holds up all of them. The other
filesystem-touching requests already avoid this: directory browsing is spawned off the loop onto a capped pool of
blocking workers with a deadline (`crates/farhelm-supervisor/src/service/core.rs:4605-4640`, dispatched at
`handlers.rs:2874-2882`), and GitHub repository search is spawned under the shared request-admission limit
(`handlers.rs:2961-2986`).

The preview is not. Its handler (`handlers.rs:2893-2935`) awaits `github_checkout_preview` (`core.rs:4494-4545`) inline.
That resolves and stats the checkout root, then calls `occupied_related_names`
(`crates/farhelm-supervisor/src/working_copies.rs:1029-1031`), which does a synchronous `std::fs::read_dir` over up to
100,000 entries on an async runtime thread, then reads the working-copy table from the store. There is no blocking pool
and no deadline. The existing Planned TODO "Keep session creation off the connection read loop" covers only create.

SPEC.md's "Waiting between operations on one host" says terminal input and output, attaching, detaching and resizing
must not wait on other slow work. On a slow or very large checkout root (an NFS or sshfs home directory, a huge folder),
each preview parks the whole connection, so typing in every terminal on that host freezes while the user types the
repository name; and on a hung mount it parks indefinitely, with a runtime thread that cannot be reclaimed. Two
reviewers flagged this. The tag is "possible" because SPEC.md's "Healthy local filesystems" explicitly accepts hangs
from a failing filesystem, which leaves the slow-but-healthy root as the remaining case, and the stall length there was
not measured.

The fix is to dispatch the preview through `spawn_admitted` like repository search, and run root resolution plus the
occupancy scan through the existing capped blocking-worker pool used by browse (`run_directory_browse_worker` /
`directory_browse_workers`) with a reply deadline. Add a test that terminal input keeps flowing while a preview is
parked.
