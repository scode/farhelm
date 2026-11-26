# The records tool silently depends on recent git versions

Reviewed commit: 7f703a44e069e2d53a626ffa0941c2ab37aa6f44

## TLDR

On a host whose git predates 2.41 (for example Apple's bundled git, which has long been on the 2.39 line, or Debian 12),
every `cargo xtask records backup` run fails and sends a "could not fetch from the records repo" notification, so the
agent-records backup the docs call load-bearing is never taken there. Nothing documents a minimum git version.

On a host with an older git (Debian 11 ships 2.30), `cargo xtask records path`/`push` quietly lose everything that is
supposed to keep the user's own git setup out of records pushes: the repo token is not sent, and the user's global git
config (credential helpers, hooks, URL rewrites) applies again. The result is a confusing authentication failure, or a
prompt from the user's own credential helper, instead of a clear "git too old" error.

## Details

Priority bucket: `other`.

Provenance: trailing code review of `953603f1a036eb5846ebf251d1564c5efd9c8e67..f0c042c9efea4a46bc02e0ad880c1a30aae3c85f`
(run `70f0f28c-538d-461e-b80e-783ddaafaadb`), reviewer records `t15a-records-tool` `f1`, `t15b-records-tool` `f3`
(Claude Code `claude -p`, `claude-opus-5-5`, effort high). Findings were validated by source and contract analysis;
nothing was built, run or reproduced.

### Recording notes

Two reviewer records merged (t15b f3 names t15a f1 as the same class: an undocumented git version floor that one version
check could cover). Maintainer tooling. Confidence: the version dependencies come from git's release notes; which
supported hosts run older git is not established. No coverage source covers it.

### Reviewer record `t15a-records-tool` `f1`: records backup requires git >= 2.41 (fetch --porcelain) and fails on every run with older git

Location: `xtask/src/records/backup.rs:96`

TLDR: On a host whose git predates 2.41 (for example Apple's bundled git, which has long been on the 2.39 line, or
Debian 12), every `cargo xtask records backup` run fails and sends a "could not fetch from the records repo"
notification, so the agent-records backup the docs call load-bearing is never taken there. Nothing documents a minimum
git version.

xtask/src/records/backup.rs:95-106 (at the reviewed commit) runs
`git fetch --porcelain --no-tags --no-write-fetch-head <remote> refs/heads/*:refs/heads/*`. `git fetch --porcelain` was
added in Git 2.41; older git rejects the unknown option with a usage error. The code then sees a non-success status with
no `!` line on stdout, so it classifies the run as Cause::Fetch (backup.rs:112-121) and notifies "could not fetch from
the records repo". That message is misleading, since the transport is fine. The rest of the tool does not need 2.41
(`git push --porcelain` in clone.rs:505-510 is old), so backup is the only affected command. `.agents/agent-records.md`,
"Backup", says backup runs "from any checkout on a host with the config's [artifacts] section and the API token file",
and "Host files" says the API token lives "only on the maintainer's own host", so the backup host is whichever machine
the maintainer uses; neither file names a git floor. Synthesis check (turn 2): the repository supports git well below
2.41 elsewhere. crates/farhelm-supervisor/src/launch.rs:2182-2185 (tip) keeps a workaround "once Git 2.55 is Farhelm's
minimum", and SPEC.md:2033 / SPEC_impl.md:3881 describe behavior on Git older than 2.48. No doc pins the maintainer's
host to a recent git. Verify: on git 2.39, `git fetch --porcelain` prints `error: unknown option 'porcelain'`. Fix:
document git >= 2.41 and check `git --version` up front with a clear message, or drop --porcelain and detect the refused
non-fast-forward another way, for example by matching the `! [rejected] ... (non-fast-forward)` stderr line under the
LC_ALL=C the tool already forces.

Suggested bucket: other. This is maintainer tooling that fails loudly (non-zero exit plus notification) and loses no
data. It would move up only if the maintainer's backup host is known to run old git and the failure went unnoticed.

Confidence: likely. Unverified premise: a host expected to run backup has git older than 2.41. That
`git fetch --porcelain` arrived in 2.41 comes from git's release notes and was not checked against a binary here (this
host has 2.43).

proposed_drop: none

possible_cover: none

### Reviewer record `t15b-records-tool` `f3`: On git older than 2.31/2.32 the records tool's isolation and token header silently vanish

Location: `xtask/src/records/git.rs:44`

TLDR: On a host with an older git (Debian 11 ships 2.30), `cargo xtask records path`/`push` quietly lose everything that
is supposed to keep the user's own git setup out of records pushes: the repo token is not sent, and the user's global
git config (credential helpers, hooks, URL rewrites) applies again. The result is a confusing authentication failure, or
a prompt from the user's own credential helper, instead of a clear "git too old" error.

At 7f703a44e0 (unchanged at tip), `child_env` (git.rs:44-73) delivers every setting the tool relies on, including the
`http.extraHeader` carrying the token and `core.hooksPath`/`credential.helper`/`core.excludesFile`, as
`GIT_CONFIG_COUNT`/`GIT_CONFIG_KEY_<i>`/`GIT_CONFIG_VALUE_<i>`, and blocks the user's global config with
`GIT_CONFIG_GLOBAL=/dev/null`. The git 2.31.0 release notes (/usr/share/doc/git/RelNotes/2.31.0.txt:127-130) introduce
the environment variable/value pairs, and 2.32.0 (/usr/share/doc/git/RelNotes/2.32.0.txt:88-93) introduces
GIT_CONFIG_GLOBAL. Older git ignores unknown environment variables, so on 2.28-2.30 (git below 2.28 already fails loudly
on `init --initial-branch`, clone.rs:860) every git child runs with the user's ~/.gitconfig and without the Bearer
header. `fetch` in `sync` (clone.rs:198-202) then gets an auth failure, or runs a global credential helper, or follows a
global `url.insteadOf`; a `push` would run a global `core.hooksPath` pre-push hook. Nothing in xtask/ checks the git
version (`git grep -i 'git version'` over xtask/ and .agents/agent-records.md finds nothing), and the module docs
(git.rs:6-21) promise isolation on hosts whose git setup the tool does not own. Fix: check `git --version` once at
startup and refuse below 2.32 with a clear message (the t15a finding suggests backup needs 2.41, so one documented floor
for the whole tool fits), and state the floor in .agents/agent-records.md.

Suggested bucket: other; the failure is in agent tooling, usually loud, and needs a host with git older than 2.32, which
the maintainer's executor hosts probably are not.

Confidence: confirmed for the version dependency (git release notes plus source); which supported hosts run git below
2.32 is not established.

proposed_drop: none

possible_cover: t15a-records-tool f1 (backup requires git >= 2.41) is the same class, an undocumented git version floor,
in a different module; one fix (a version check) could cover both.
