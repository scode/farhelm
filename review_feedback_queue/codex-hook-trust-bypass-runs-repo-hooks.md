# Codex hook-trust bypass lets repository hooks run unreviewed

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Opening a Codex session on a repository Farhelm just cloned can run the repository author's hook commands with no
approval prompt.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0508-2b597e9-5788` (audit of Area 6, supervisor session state machine) as
`F24 / SEC-CODEX-HOOK-TRUST`, tagged **possible**. Anchors and title: `agent_kind/mod.rs:1385`, `service/core.rs:2621`,
`agent_kind/mod.rs:2739` — Every Codex launch bypasses Codex's per-hook review, so a cloned repository's own hooks can
run without approval

Farhelm learns which Codex conversation a session belongs to by adding its own "SessionStart" hook to the Codex command
line. A hook is a shell command that Codex runs at certain points; this one reports the conversation id back to Farhelm.
Codex normally will not run a hook until the user has reviewed and approved it in a startup dialog. That dialog would
block an unattended launch, so `CodexIntegration::hook_argv` (`agent_kind/mod.rs:1385`) also passes
`--dangerously-bypass-hook-trust`. The flag switches hook review off for the whole process, not only for Farhelm's hook.
The docstring there and SPEC_impl.md ("Supervisor internals", around the hook-injection paragraph) accept one named
cost: an unapproved hook in the user's own Codex configuration home (`~/.codex` or `$CODEX_HOME`) will run too.

The reviewer argues the exposure is wider than that. Codex treats hooks that ship inside a repository
(`<repo>/.codex/hooks.json`, or `[hooks]` in `<repo>/.codex/config.toml`) with two separate gates. The folder must be
trusted before its `.codex/` configuration loads at all, and then each hook must be approved individually. The bypass
flag removes the second gate. Farhelm routinely clears the first. When a user makes a structured Codex launch with
workspace trust set to true, the helm puts a `{codex:trusted-cwd}` placeholder into the command. `fill_cwd`
(`agent_kind/mod.rs:2739`) turns that placeholder into a per-run `projects={"<cwd>"={trust_level="trusted"}}` override.
SPEC.md (search options, around line 345) says this also applies to a fresh checkout that Farhelm itself just cloned
from GitHub. A user who answers Codex's own "trust this folder" prompt clears the first gate the same way. In
`with_hook_argv_using` (`service/core.rs:2621`), a Codex launch skips hook injection only when its argv contains a bare
`--`, when the argv already configures Codex hooks itself, or when injection is turned off through the
`FARHELM_AGENT_HOOKS` opt-out. I confirmed that none of these checks looks at whether the launch trusts the folder or
whether the folder has its own hook files. Under this reading, a repository author's hook commands would run as the
user, outside Codex's sandbox and without approval, as soon as the user types the first prompt.

This matters because the SPEC accepts only unreviewed hooks that the user placed in their own configuration. A
repository's hooks are third-party code, and Codex's per-hook review exists to stop exactly that code. Trusting a folder
so the agent may work in it is a different decision from approving the folder's hook commands. The fixes, simplest
first:

- Skip hook injection, with the usual logged reason, when the launch carries the trusted-workspace override or the
  folder contains `.codex/hooks.json` or `.codex/config.toml`.
- Scope trust to Farhelm's own hook definition, if Codex supports that.
- At minimum, have the maintainer sign off on the wider cost and document it in SPEC_impl.md and the user docs.

Before choosing, test against the pinned Codex version with a throwaway repository whose `SessionStart` hook writes a
marker file.

Restater note: The Farhelm half is confirmed in code: the bypass is added to every injected Codex launch, the
trusted-folder override exists, and nothing skips injection for trusted or hook-bearing folders. The Codex half is not
verified here. That Codex loads repository hooks once the folder is trusted, and that this flag then skips review for
them, rests on public Codex docs and issues (openai/codex#32491, #24093) that the reviewer cited. It was not reproduced
against the pinned Codex version, and I did not run Codex.
