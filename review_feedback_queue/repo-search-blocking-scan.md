# GitHub repository search can tie up the whole supervisor on a slow checkout folder

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

If the checkout folder sits on a slow network share, a few repository searches from the create dialog can stall the
whole supervisor on that host: terminals, status updates and every request stop until the filesystem answers.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F6 / COR-REPOSEARCH-BLOCKING`, tagged **possible**. Anchor and title:
`crates/farhelm-supervisor/src/repository_discovery.rs:134` — GitHub repository search lists and stats the checkout
folder synchronously on async workers, which its 5-second timeout cannot interrupt.

The create dialog's GitHub repository search asks the supervisor which repositories are already cloned under the
checkout root. Unlike the preview in F5, this request is correctly spawned off the connection's read loop, under the
shared admission limit of 8 concurrent handlers, and wrapped in a 5-second timeout (`github_repo_search`,
`crates/farhelm-supervisor/src/service/core.rs:4455-4475`). But the scan itself (`RepositoryScanner::scan`,
`crates/farhelm-supervisor/src/repository_discovery.rs:134-166`) calls blocking filesystem functions directly on the
async runtime's worker threads: `std::fs::read_dir` on the root, `file_type()` per entry, and `symlink_metadata` of each
child's `.git` (`has_local_git_dir_or_file`, `:421-429`). Only the per-repository git origin read is truly async. An
async timeout can fire only when the task reaches an await point, and the scan's own deadline check runs between
entries, so a single stuck system call is never interrupted.

If the checkout root passes the initial async canonicalize/stat but listing it or stat-ing its children stalls (a slow
network share, automounted child directories), each search pins one runtime worker and one admission slot. The runtime
has roughly one worker per CPU and admission allows 8, so a few searches can occupy every worker and stop the whole
supervisor: terminal I/O, its periodic status ticker, and all requests. It is tagged "possible" because it is open
whether real roots show the "root check succeeds, then listing stalls" pattern; a fully hung mount would already time
out earlier.

The suggested fix is to run the directory enumeration and per-child stats through the same capped blocking-worker pool
that directory browse uses (`core.rs:1307-1312`, `:4605-4640`), or a sibling pool, keeping only the git child-process
reads async.
