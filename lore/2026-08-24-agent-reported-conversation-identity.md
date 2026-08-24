# Agent-reported conversation identity: SPEC's observation-only rule bends, and protocol 12

Decision, 2026-08-24, while planning the fix for "resume lands in the wrong conversation after `/clear`": SPEC.md's
"capture works purely by observing the agent from the outside ... never by configuring the agent" is reworded to forbid
writing to the agent's own configuration or record directories, while explicitly allowing a hook passed on the command
line for one launch — because such a hook writes nothing the vendor owns (no configuration file, no conversation record,
no trust state) and cannot outlive the launch that carried it. It is not traceless in general: the report lands in
farhelm's own database and every hook run leaves a line in farhelm's own hook log. The record scan stays as the fallback
whenever no report has been accepted, so SPEC.md's older promise that agent-configuring integrations "must never be
required" survives untouched. In the same breath, the `ReportConversation`/`ConversationReported` message pair takes
`PROTOCOL_VERSION` from 11 to 12.

## What forced it

Capture correlates a session with an agent conversation by scanning the agent's own record directory in a window around
the session's first keystroke. The claim it produces is write-once by design: confirmable, never replaceable. That was
built on an audited fact — a plain resume appends under the same id, and a new id appears only on an explicit fork — and
that fact stopped being the whole story. Claude Code's `/clear` and Codex's `/new` both start a new conversation, with a
new id, inside the same process, and the record for it is a new file with no pointer back to the old one. Nothing
observable from the outside distinguishes "the user threw that conversation away" from "the user is still in it". So
after a `/clear`, "resume conversation" faithfully resumes the conversation you cleared.

There is no observation-only fix. The information simply is not on disk. Either the agent tells us, or the feature stays
broken for anyone who clears a context — which is everyone, routinely.

## The alternatives as they looked

Both vendors fire a `SessionStart` hook whose payload carries the exact id a resume needs, and both accept a hook
supplied on the command line. Three ways to get one attached, verified by hand on 2026-08-24 against Claude Code 2.1.241
and Codex CLI 0.149.x:

1. **Write a hook into the user's own config** (`~/.claude/settings.json`, `~/.codex/config.toml`). Persistent, survives
   every launch, and needs no per-launch flags. Rejected: it is exactly the thing SPEC.md forbids, and for Codex it
   additionally means computing and writing the vendor's own trust hash into the user's config file. Farhelm would be
   mutating state it does not own, that outlives the session, and that a user would have to know to clean up.
2. **A per-launch command-line hook.** Claude takes `--settings <inline json>`; Codex takes
   `-c features.hooks=true -c hooks.SessionStart=…` plus `--dangerously-bypass-hook-trust`, which is the only per-launch
   trust bypass it offers. Nothing is written into anything the vendor owns — not its config, not its conversation
   records, not its trust store — and the flags die with the process; what the report produces is written on farhelm's
   own side of the line, in its database and its hook log. Chosen.
3. **Do nothing for Codex** and hook only Claude, avoiding the trust bypass. Rejected as a default: Codex `/new` would
   stay broken. It survives as the per-kind opt-out instead (`FARHELM_AGENT_HOOKS`), which is the same outcome for
   anyone who wants it and no outcome at all for anyone who does not.

The SPEC rewording is the narrowest one that admits option 2 and still excludes option 1. The line it draws is
persistence, not mechanism: what the rule was ever protecting is that farhelm not leave configuration behind in an
agent's home directory, where it outlives the session, survives farhelm's own removal, and changes how the agent behaves
when the user runs it themselves. A flag on one command line does none of that.

Two costs were accepted with the choice rather than designed around, both of them borne only by the launches that
actually receive the injected flags — a skipped or opted-out launch carries no bypass and pays neither. Codex prints a
hook-trust warning line above its composer on such a launch, which is a vendor's own output and therefore the one
permitted exception to the rule that nothing farhelm attaches to a launch may be visible inside the session; that rule
went into SPEC.md in the same edit, because a per-launch injection surface needs a stated no-errors policy more than the
scan ever did. And the bypass flag means any hook a user has in `~/.codex` but has not trusted also runs during a
farhelm launch — documented prominently in README.md at the maintainer's insistence, not buried in the docs page,
because it is a security-relevant consequence of a flag the user did not choose.

## Why the report may overrule the scan

The one guarantee this deliberately weakens: an ambiguous capture verdict was durable and dominated everything, on the
reasoning that scan evidence which could not distinguish two candidates must never be resolved by later, equally weak
evidence. A report is not scan evidence. It is the agent's own answer, delivered from inside the process that minted the
id. So a report clears ambiguity and replaces a scan claim, and only another report replaces a report — which is exactly
what a second `/clear` is.

The trust boundary widens, and that was weighed rather than overlooked. Every process in the session's tree holds the
session credential, so any of them can now replace the resume identity, where before it could only create child
sessions. It is the same boundary the scan already had — the agent writes the very records the scan reads — and the
mitigations are the same shape: the id must pass the existing plausibility check, and resume argv is slot-substituted
rather than spliced into a command string. What those mitigations buy must be stated precisely: they stop a hostile id
from becoming anything other than an id — a relaunch can never be made to run something — but they do not stop a
credential holder from redirecting resume to some other plausible id it knows, including another existing conversation
of the same user. Which conversation gets resumed is, and always was, the agent's call; that was accepted with the
boundary, not missed.

## Protocol 12

`ReportConversation` and `ConversationReported` are two new tagged `ControlMsg` variants, and by the rule this protocol
has followed since version 4 — an unrecognized tag is connection-fatal, never a tolerated no-op — that is a bump, not an
additive change. Skew on that particular edge is only ever transient: the supervisor holds its own binary's path from
startup and injects that path as the hook, so an in-place upgrade leaves an old supervisor invoking a new binary (or the
reverse) until it restarts. The handshake catches it — a mismatched version is refused before any report is accepted,
the hook logs the failure and exits 0, and the record scan carries the session as it always did. The helm's hello checks
the same number, and there a mixed fleet is the normal steady state rather than a window. Version 12 it is, with the pin
test renamed as every prior bump has done.

Recorded here rather than appended to the frozen per-version changelog from 2026-08-20, per this directory's rule that
entries are written once and never maintained.
