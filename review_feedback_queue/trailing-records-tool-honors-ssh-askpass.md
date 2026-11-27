# The records tool's git children and its credential-helper test honor SSH_ASKPASS

Reviewed commit: 7f703a44e069e2d53a626ffa0941c2ab37aa6f44

## TLDR

On a host whose agent environment sets SSH_ASKPASS with a usable display (common on some Linux desktops), a records
token the server rejects makes `cargo xtask records path`/`push` show a graphical password prompt and wait for it
instead of failing. Nobody is watching an unattended executor or landing monitor, so it stalls indefinitely while
holding the slot clone's lock, which also blocks every later records call from that slot.

On a machine whose session sets SSH_ASKPASS (common on Linux desktops with an ssh-askpass package, and inherited by
agents started from that session), the credential helper's test asks git for a github.com credential, and git pops up a
graphical password prompt instead of failing. The test then waits until someone answers or closes the dialog, and if the
dialog returns text the test's "git fill fails for a foreign host" assertion breaks.

## Details

Priority bucket: `other`.

Provenance: trailing code review of `953603f1a036eb5846ebf251d1564c5efd9c8e67..f0c042c9efea4a46bc02e0ad880c1a30aae3c85f`
(run `70f0f28c-538d-461e-b80e-783ddaafaadb`), reviewer records `t15b-records-tool` `f2`, `t15a-records-tool` `f2`
(Claude Code `claude -p`, `claude-opus-5-5`, effort high). Findings were validated by source and contract analysis;
nothing was built, run or reproduced.

### Recording notes

Two reviewer records merged: the same git prompt-order mechanism at two sites (the tool's git children in
`xtask/src/records/git.rs` and the isolated environment in `scripts/test-records-git-credential.py`). Maintainer
tooling. t15b f2 is `likely`: whether Cloudflare Artifacts answers a rejected token with 401 (which starts git's prompt)
is not established. No coverage source covers it.

### Reviewer record `t15b-records-tool` `f2`: Records git children still honor SSH_ASKPASS, so a rejected token can open a password dialog and hang

Location: `xtask/src/records/git.rs:55`

TLDR: On a host whose agent environment sets SSH_ASKPASS with a usable display (common on some Linux desktops), a
records token the server rejects makes `cargo xtask records path`/`push` show a graphical password prompt and wait for
it instead of failing. Nobody is watching an unattended executor or landing monitor, so it stalls indefinitely while
holding the slot clone's lock, which also blocks every later records call from that slot.

Revision (turn 2): git behaviour now checked against the installed git 2.43 documentation, and the route premises
narrowed. Content otherwise as before.

At 7f703a44e0 (unchanged at tip), `Git::at` (git.rs:83-100) strips inherited `GIT_*` variables and `child_env`
(git.rs:44-73) sets `GIT_TERMINAL_PROMPT=0` and `credential.helper=` with a null stdin; the module docs (git.rs:21)
state "nothing may ever wait for a password". gitcredentials(7), "Requesting credentials" (git 2.43 on this machine),
says that without a helper git tries GIT_ASKPASS, then core.askPass, then the SSH_ASKPASS environment variable, and only
then the terminal; git(1) documents GIT_TERMINAL_PROMPT as only disabling the terminal prompt. `SSH_ASKPASS` does not
start with `GIT_`, so it survives the sweep, and no records code sets GIT_ASKPASS or core.askPass (`git grep -i askpass`
over xtask/ finds nothing). Farhelm itself sets SSH_ASKPASS only on the helm's own ssh children
(crates/farhelm-helm/src/ssh_sign_in.rs:233), not on sessions, so the variable would have to come from the host's own
environment. Route: a token is rejected with HTTP 401 -> git runs credential_fill with no helper -> askpass program ->
dialog waits; meanwhile `SlotClone::open` holds `.git/records.lock` (clone.rs:130-138) for the whole run (callers:
mod.rs:161-163 and 201-203). http.lowSpeedLimit does not help, since no transfer is in progress. Fix: set `GIT_ASKPASS`
to an empty value in `child_env` (git treats a set but empty GIT_ASKPASS as no askpass and does not fall back to
SSH_ASKPASS; alternatively env_remove SSH_ASKPASS and set core.askPass to empty in FIXED_CONFIG), and extend
`isolation_settings_are_always_present` to assert it.

Suggested bucket: other; harm is confined to agent tooling (a stalled unattended flow), and it needs both a rejected
token and SSH_ASKPASS plus a display in the agents' environment. It would move up only if records stalls are shown to
block user-facing work.

Confidence: likely. The git prompt order is confirmed from the git documentation. Unverified premises: (1) Cloudflare
Artifacts answers an expired or revoked Bearer token with 401 (which starts git's credential prompt) rather than 403
(which does not); the attended clone's credential helper only works because git asks for credentials after a 401 on
unauthenticated requests, which suggests 401 but does not prove it for a rejected token; (2) some supported host runs
executors with SSH_ASKPASS and a display in their environment. Neither can be settled by reading source here.

proposed_drop: none

possible_cover: none

### Reviewer record `t15a-records-tool` `f2`: credential helper's clone test can open an SSH_ASKPASS dialog and hang: its isolated env keeps SSH_ASKPASS

Location: `scripts/test-records-git-credential.py:93`

TLDR: On a machine whose session sets SSH_ASKPASS (common on Linux desktops with an ssh-askpass package, and inherited
by agents started from that session), the credential helper's test asks git for a github.com credential, and git pops up
a graphical password prompt instead of failing. The test then waits until someone answers or closes the dialog, and if
the dialog returns text the test's "git fill fails for a foreign host" assertion breaks.

scripts/test-records-git-credential.py (reviewed commit, snapshot lines 92-94 and 120-123):
`test_attended_clone_command_configures_a_working_helper` builds the git environment as the inherited environment minus
`GIT_*`, plus GIT_CONFIG_NOSYSTEM=1, GIT_CONFIG_GLOBAL=/dev/null and GIT_TERMINAL_PROMPT=0. It then runs
`git credential fill` for `host=github.com`, expecting a non-zero exit "with prompting off" (comment at snapshot line
120). GIT_TERMINAL_PROMPT=0 disables only the terminal prompt. Git's prompt path (prompt.c, git_prompt with
PROMPT_ASKPASS, used by credential.c's credential_ask_one) tries GIT_ASKPASS, then core.askPass, then SSH_ASKPASS before
checking GIT_TERMINAL_PROMPT. Removing `GIT_*` drops GIT_ASKPASS and the null global config drops core.askPass, but
SSH_ASKPASS survives, so git runs that program for the Username prompt. The docstring's claim that the environment is
"isolated from the user's git config" therefore does not cover the prompt. Fix: also drop SSH_ASKPASS from `env` (and
set GIT_ASKPASS to a non-interactive program such as /bin/false, or set SSH_ASKPASS_REQUIRE=never), so the foreign-host
fill fails immediately everywhere. Verify by reading git's prompt.c git_prompt; no run was made.

Suggested bucket: other. A maintainer test can hang or fail spuriously on some desktops; no user or records data is
affected.

Confidence: confirmed for the mechanism. The installed git documentation sets the order: gitcredentials(7) asks through
GIT_ASKPASS, then core.askPass, then "if the SSH_ASKPASS environment variable is set, its value is used", then
"Otherwise, the user is prompted on the terminal". git(1) says GIT_TERMINAL_PROMPT=false only means "git will not prompt
on the terminal". Revised turn 3. Still unverified: whether a machine that runs this test sets SSH_ASKPASS.

proposed_drop: none

possible_cover: t15b-records-tool f2 (xtask/src/records/git.rs:55) reports the same SSH_ASKPASS mechanism for the
records tool's own git children. That is a different file and fix, so it covers the mechanism, not this test.
