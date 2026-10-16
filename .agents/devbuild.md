# Trying a PR or branch on the maintainer's Mac (devbuild)

The root `AGENTS.md` section "Trying a PR or branch on the maintainer's Mac", moved here unchanged. Read it for
"devbuild <thing to test>" and for any other request to try a PR or branch on the maintainer's Mac, or to run, test or
see it there.

> NOTE: In an unattended run (plan executor, landing monitor, delegate), an instruction here to ask, confirm with, or
> propose to the maintainer follows `AGENTS.md`, "Unattended runs".

## Trying a PR or branch on the maintainer's Mac

"devbuild <thing to test>" means: give the maintainer the line below for that thing. The thing is a PR number, "this PR"
(resolve it from the current bookmark or branch with `gh pr view`; if there is no PR, ask rather than guessing one), or
a branch name such as `main`. Treat any other request to try a PR or branch on their Mac, or to run, test or see it
there, the same way. Put the PR's number or the branch's name in place of `<N>`:

```
curl -fsSL https://raw.githubusercontent.com/scode/farhelm/main/scripts/devbuild-github.sh | bash -s -- <N>
```

`main` is a branch like any other, so `bash -s -- main` builds the current main. The script checks the target out in
`/tmp/farhelm-devbuild` on the Mac, builds it with `scripts/devbuild-mac.sh`, and ends by printing the command that
starts the result. Read `scripts/devbuild-github.sh` for what it does and refuses. Make sure GitHub has what you want
tried first: the script builds the pushed head, not your working copy, so push before handing out the line and say so if
you have unpushed changes. The target must contain `scripts/devbuild-mac.sh`; an older base needs a rebase first. The
script refuses a PR whose branch is not in `scode/farhelm`, and it takes no commit hashes. The line is the same every
time, and rerunning it is always safe, so after another push the maintainer reruns the same line.

NOTE: The build runs on the maintainer's real Farhelm state, and its supervisor applies any database migration the
target adds. A release that predates the migration then refuses that database, and another unlanded migration with the
same number can misread it. When the target changes a supervisor or helm database schema, say so plainly when handing
out the line, and offer the variant that keeps the build on separate state, which the build remembers and the start
command it prints uses. The directory is per target, because two targets' unlanded migrations can collide there just as
on the real state:

```
curl -fsSL https://raw.githubusercontent.com/scode/farhelm/main/scripts/devbuild-github.sh | FARHELM_DESKTOP_STATE_DIR=/tmp/farhelm-devbuild-state-<N> bash -s -- <N>
```
