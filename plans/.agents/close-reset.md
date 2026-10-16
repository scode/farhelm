# Clean main and closing a plan

The full clean-main boundary: the starting check, the reset after a close, the narrower reset for a released claim, and
pruning build leftovers. `plans/AGENTS.md`, "Clean-main boundary", carries the starting check and the core of the close
rule inline; this file has the whole text. Executors and drainers read it before their first action, because a block or
a give-back can come at any point of a plan; the landing monitor reads it before its first action too, since it resets
the way an executor's close does.

## Clean-main boundary

Every planning-system flow starts from a clean working copy of the latest `main`. Run `jj status` first. If it reports
changes, abort the flow and tell the maintainer which paths are dirty; do not stash, commit, claim a plan, or carry
those changes into a plan checkout. Once the status is clean, fetch `origin`, start an empty working copy from
`main@origin` with `jj new main@origin`, and verify clean status again before planning, picking, executing, draining,
reviewing, or landing plans.

Whenever work on a plan ends, the executor ALWAYS returns to a clean working copy of the latest `main`, inside a drain
or not, and whether the plan delivered, blocked, or gave up its claim. Unlike the starting check, this one does not
abort on a dirty working copy: it cleans it up. When the executor closes a plan it still held (it delivered, blocked, or
gave the claim back itself after pushing), every consistent piece of the plan has been pushed (steps 9 to 11 under
`plans/.agents/executing.md`, "Executing one plan"), so anything still local is a leftover (a half-made edit, a scratch
file jj snapshotted, an unpushed or amended commit on the plan's stack), and leaving it in place is how the next plan
ends up building on top of it. The starting check is different because a working copy found dirty before any plan runs
may be the maintainer's own work or an interrupted executor's unpushed progress, and neither is the executor's to throw
away.

Here's the reset after a close, run from the checkout, with `<slug>` the plan's slug:

```
jj git fetch
s='::@ & mutable() & ~::remote_bookmarks()'
jj diff --summary -r "($s) ~ ::(($s):: ~ ($s))"
jj abandon --retain-bookmarks -r "($s) ~ ::(($s):: ~ ($s))"
```

Then put each local `plan/<slug>/*` bookmark back where GitHub has it, and move onto `main`:

- a bookmark with an `@origin`: `jj bookmark set <name> -r <name>@origin --allow-backwards`;
- a bookmark that was never pushed: `jj bookmark delete <name>`;
- finally `jj new main@origin` and `jj status`, which must report no changes and no conflicted `plan/<slug>/*` bookmark.

The fetch comes first because `remote_bookmarks()` is only as current as the last fetch, and a commit that reached
GitHub since then must not count as unpushed. `$s` is the working copy and its ancestors that no remote bookmark
reaches; the `~ ::(($s):: ~ ($s))` part leaves out any of those that something outside the set builds on (another jj
workspace's working copy, another plan's commit), because abandoning them would rewrite that history under its owner.
What the guard leaves out stays where it is, outside the working copy. The `jj diff` lists the paths being dropped, and
the plan's closing log entry records them so a discard is never silent. The bookmark step is needed because
`--retain-bookmarks` only moves a bookmark to the abandoned commit's parent: that is its pushed commit when the local
work merely extended the stack, but `main` when the pushed commit was amended or rebased, and a later push of that name
would then empty the PR. Without `--retain-bookmarks` the abandon deletes the bookmark outright, which is worse.

When the claim was released rather than given up (`plans/.agents/executing.md`, "Executing one plan", step 1, or
`block`/`deliver` exiting 10), the executor no longer owns the plan and the reset is narrower: `jj git fetch`, then
`jj new main@origin` and `jj status`, with nothing abandoned and no bookmark touched. Unpushed work may still be local
at that point (`plans/.agents/executing.md`, "Executing one plan", step 9 forbids pushing it once the claim is gone),
and step 7 there has the next claimer inspect exactly that work in this checkout before deciding what to do with it.
Moving off it keeps the working copy clean without destroying it; record the old working-copy commit id in the executor
log so the leftover can be found.

Either reset leaves ignored files alone, since jj never sees them. That keeps the checkout's build caches (`target/`,
`node_modules/`, `.ci-tmux/`, and the like) for the next plan, and it also leaves the other ignored files (release
drafts, delegation records, saved authentication state) where their owners expect them. A monitor's baseline commit is
not a reason to skip the reset.

### Pruning build leftovers

NOTE: This is not a `cargo clean`. The point is to keep the next plan's build incremental, so everything it would reuse
stays: compiled third-party dependencies, build-script outputs, `.ci-tmux/`, `.ci-nextest/`, `node_modules/`, and the dx
web build.

Because the resets leave ignored files alone, a checkout that runs plan after plan accumulates build output nobody will
use again, and an executor checkout has been seen to reach well over a hundred gigabytes this way. Most of it is rustc's
incremental cache. Rustc keeps one directory per crate and build configuration under `target/<profile>/incremental/`
(and `target/<triple>/<profile>/incremental/`), and every change to features, flags or dependency versions starts a new
one; rustc cleans up old sessions inside a directory, but never a directory whose configuration stopped being built.
Next is the workspace's own crates in `target/<profile>/deps/`: every rebuild of a `farhelm*` crate or test binary under
a new hash leaves the old one behind.

So `plans/.agents/executing.md`, "Executing one plan", step 6 marks the start of the plan's builds with
`mkdir -p target && touch target/.plan-start`, and after every close (`plans/.agents/executing.md`, "Executing one
plan", step 12, or the reset for a released claim), once nothing the plan started is still building or testing in this
checkout, the executor prunes relative to that mark:

- every directory directly inside an `incremental/` directory anywhere under `target/` that is not newer than
  `target/.plan-start`;
- files under `target/**/deps/` that belong to a workspace target (names from
  `cargo metadata --no-deps --format-version 1`, with `-` spelled `_`, matched as `[lib]<name>-<16 hex digits>[.ext]`)
  and are not newer than the mark. Anything this removes that was still current is a workspace crate, which most plans
  rebuild anyway once `main` has moved, and cargo rebuilds a target whose output is missing. Never prune third-party
  artifacts by age: a dependency that has not changed is never rewritten, so an old file is usually a current one, and
  losing them is the full rebuild this section exists to avoid;
- separate cargo target directories the plan made for one check, whether inside `target/` (a subdirectory that holds its
  own `CACHEDIR.TAG`, such as an isolated `CARGO_TARGET_DIR` for the desktop smoke or an asset check) or in the
  executor's scratch space, removed whole;
- one-off outputs the plan produced: `e2e/test-results/`, `e2e/playwright-report/`, `website/dist/`, `website/.astro/`,
  `target/tmp/`, and capture outputs such as `target/readme-hero/`, `target/readme-video/` and `target/dist-desktop/`.
  Leave any of these that the plan's working log cites as retained failure evidence (`.agents/test-evidence.md`).

If `target/.plan-start` is missing (a plan claimed before this rule existed), skip the two age-based items and do the
rest. Record in the plan's closing log entry roughly how much the prune freed. Nothing here touches the other ignored
files the reset paragraph in `plans/.agents/close-reset.md`, "Clean-main boundary", names, the recorder's run records
(which live outside the checkout), or anything in another checkout.
