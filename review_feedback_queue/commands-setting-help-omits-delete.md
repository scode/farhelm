# The "run farhelm commands without asking" help omits that agents may delete sessions and templates

Reviewed commit: 08ab6e08f9ddd40b5494f2cbbeaaeb9d4e3332b7

## TLDR

In a host's settings, the explanation under "Run farhelm commands without asking" lists what turning it on gives away:
starting, stopping, restarting, renaming and cloning sessions on any host, and editing launch templates. It leaves out
that agents on that host can also delete sessions on any host without asking (stopping their agent and terminal tabs and
moving their managed checkout to the trash) and delete launch templates. A user deciding whether to grant the setting is
not told about the most destructive thing it allows.

## Details

Found by the 2026-10-22 prioritized persistent review, group `164-ui-hosts-6of6` (finding f1).

- `commands_help` (`crates/farhelm-ui/src/hosts/settings_dialog.rs:261-271`): the "on" text is "Agents in this host's
  sessions start, stop, restart, rename and clone sessions on any host, and edit launch templates, without asking you."
  The "off" text names the same verbs. Its doc says the "on" text "names what turning it on gives away". The checkbox
  tooltip (around line 622, "start, stop and change sessions and templates") has the same gap.
- `crates/farhelm-helm/src/approvals.rs:368`:
  `_ if row.commands_without_asking && !direct_allow => return Ok(Allowed::Raw)` skips the card for every non-link
  action whose `requires_direct_allow()` is false, and `crates/farhelm-proto/src/approvals.rs:246-254` lists `Delete`
  and `TemplateDelete` among those. So with the setting on, `farhelm agent delete --session <id> [--stop-if-running]`
  against any host's session, and a template delete, run without a card.
- SPEC.md "Agent-spawned sessions" (around 2502-2511) lists `delete` among the acting verbs the setting waives, so the
  authority itself is specified; only its explanation is incomplete. SPEC.md "Local authority and trust between hosts"
  (around 3247-3249) also leaves delete out of its summary list, which may be where the UI text came from.

Trigger: ordinary use. A user reads this explanation, the only one the GUI gives at that point, to decide whether to
turn the setting on.

How to verify: compare `commands_help` with `ApprovalAction::requires_direct_allow` and the approval gate; with the
setting on, have an agent run `farhelm agent delete --session <another host's session> --stop-if-running` and see that
no card appears.

Fix: name delete in both texts (for example "start, stop, restart, rename, clone and delete sessions on any host, and
edit or delete launch templates"), update the tooltip, and consider adding deleting to SPEC.md's summary list. The help
also omits sending prompts to running agents (`prompt`), which the setting waives too; recording noticed this while
checking and did not trace it further.

Suggested bucket: high, possibly highest. The reviewer raised highest because this is the consent text for a security
setting and review_feedback_queue/AGENTS.md treats a documentation error as highest when it could lead a reader into a
security or data-loss mistake. Recording chose high because the text already warns that agents can stop sessions on any
host and edit templates (which can already wipe a template's contents); triage should decide.

Confidence: confirmed by code reading (UI text against the helm's approval gate); not run.

Reviewer's proposed drop, kept for triage: the granted authority is specified and only its explanation is incomplete,
which may be judged wording outside the review's charter. That is not a coverage source, so recording did not drop it.

Possible cover: none found. SPEC.md "Agent-spawned sessions" and "Local authority and trust between hosts" specify the
authority, not the help text; no queue item, TRIAGE_OUTCOMES.md heading, BUGS.md entry or FILTER.md filter matches.
