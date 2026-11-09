# Tmux recognition has an unresolved executable-name boundary

Reviewed commit: 5f235d92f7184cdf949cf9d45e86dc19e71ff4df

## TLDR

Cleanup might miss a session-started tmux server under a nonstandard executable name. A supported installation producing
the required name and truncated role has not been established, so this is an investigated uncertainty.

## Details

Suggested bucket: **other**. Confidence: **unverified after investigation**. A supported producer and actual
surviving-job route would justify high; no security or user-work-loss route is established.

`crates/farhelm-supervisor/src/scope.rs:176-187` recognizes a process name ending in `: server` or an executable
basename beginning with `tmux`. A long nonprefixed real executable with a truncated role can miss both tests.
[tmux 3.4 task-name formatting](https://github.com/tmux/tmux/blob/3.4/compat/setproctitle.c#L29-L45),
[program-name lookup](https://github.com/tmux/tmux/blob/3.4/compat/getprogname.c#L23-L42) and
[role assignment](https://github.com/tmux/tmux/blob/3.4/proc.c#L183-L189) establish this naming mechanism. The existing
recognition tests cover long prefixed images and short renamed roles, not this combination (`scope.rs:1764-1778`).

The missing premise is a supported stock package or installation actually presenting such a nonprefixed executable, or
an authoritative statement that independently renamed agent-run tmux is within the cleanup promise. The inspected
[NixOS package](https://github.com/NixOS/nixpkgs/blob/nixos-26.05/pkgs/by-name/tm/tmux/package.nix#L26-L70) does not
wrap or rename tmux; **no known NixOS `.tmux-wrapped` failure is claimed**. Source/spec searches and stock-package
inspection did not settle the producer. No runtime construction was performed. A qualifying package naming contract or
clarified support boundary would settle it.

`SPEC.md:4772-4778` at the reviewed commit restricts implementation tmux to stock or shipped binaries and excludes
custom programs. Its application to an agent's independently started program is unclear alongside the ordinary user-tmux
cleanup promise (`SPEC.md:1854-1856`, 4700-4710). Do not turn that ambiguity into either confirmed product failure or
accepted harmful behavior. Current role recognition is the established source limitation.

Possible coverage: no exact queue, ledger, known-bug or unlanded-plan cover found; Planned was empty. No filter clearly
covers the full conditional lasting-cleanup consequence. Preserve the applicability caveat for triage.

Provenance: run `ee465fa5-0b29-4ce7-b1fe-8fabdb679cfe`, `cleanup-scope/f3`; primary-source and stock-package checks,
original naming hypothesis, corrections and missing premises remain in the findings history.
