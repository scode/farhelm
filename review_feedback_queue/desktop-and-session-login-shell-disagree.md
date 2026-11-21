# Desktop and session shell selection can disagree

Reviewed commit: 23ae2b9850c539d6d90284dd48322a98af7d78de

## TLDR

When `SHELL` is absent, the app and a new session can use different tool paths if their account lookups disagree.

## Details

**other**. Confirmed by source and contract inspection.

Confirmed source difference: desktop lib.rs:191-198 uses the native passwd query first. The managed supervisor inherits
the desktop's resolved tool PATH and gets the earlier PATH separately for the terminal child (lib.rs:1350-1388). It gets
no shell-selection handoff. Supervisor launch.rs:316-350, :378-404 and :421-446 resolve on each launch and prefer a
by-name getent result, then use the bounded native fallback. Restoring the terminal child's PATH occurs after that
selection (host_launch.rs:461-506). The shells differ if these lookup paths give different usable results or only one
succeeds without a matching fallback. The source establishes that condition and its effect, with no stock account
backend disagreement or rate observed. SPEC_impl.md:7486-7499 requires the same interactive login shell and explicitly
accepts the existing environment of a reused supervisor. SPEC.md:3778-3788 requires each session to see updated startup
configuration. Synthesis with path-notices/f1: a successful desktop probe reports success even when selecting a
different shell, since the notice covers the probe result. Synthesis with shell-runtime/f2 and /f7: the later
first-class Bash and Zsh launches use literal by-name programs (helm launches.rs:934-950). That inner program lookup
happens under the session's own startup environment and is distinct from selecting the outer login shell, so desktop
probe success is no guarantee the selected Bash or Zsh is available. Estimated bucket: other for the established
lookup-policy mismatch. A material failure on a supported stock configuration is not established. Possible_cover: none.
No correction to the two selection paths through 8bcca67df1afe6eb0f184ef73b9952ed7ebb329f.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `desktop-path/f1`.
