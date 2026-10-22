# The CentOS test's ssh-config cleanup deletes everything after a start marker that has no end marker

Reviewed commit: 08ab6e08f9ddd40b5494f2cbbeaaeb9d4e3332b7

## TLDR

If the user's ssh configuration holds a leftover start line from the CentOS provisioning test without its matching end
line, the next run of that test silently removes everything after that line from the user's real ssh configuration. The
test puts its block at the top of the file, so that can be the user's whole configuration.

## Details

Found by the 2026-10-22 prioritized persistent review, group `245-test-scripts-11of17` (finding f2).

Mechanism:

- `scripts/test-provision-centos.sh:177-182` (this run's block, or a named stale one) and `:357-362` (the old-format
  block) scrub with an awk program that sets `skip = 1` on the exact begin line and clears it only on the exact end
  line. The result is then published over the user's `~/.ssh/config` through `replace_ssh_config` (`:148-159`). Nothing
  checks that the end marker follows the begin marker, so a begin line without an end line drops every following line to
  the end of the file.
- The stale-block sweep at `:334-350` runs this automatically at every setup for any block whose `IdentityFile` no
  longer exists or which names none. `:354-363` always runs it for an old-format begin line.
- The block is prepended (`:387-406`), so the dropped tail is the user's own configuration.
- Interaction noted by the reviewer: for a begin line with no end line, the sweep's identity lookup (`:337-342`) reads
  the first `IdentityFile` from the user's own later stanzas. A `~/...` path there is not expanded by `[ -e ]`, so the
  block reads as stale and is scrubbed.

Trigger: a begin line without its end line. The current script never writes that itself, since both markers go out in
one atomic publish (`:387-406`). It can come from a hand edit that deletes part of a stray block, or from a merge or
sync of a dotfiles-managed config. For the old-format block, it would also happen if the old version's end marker was
not exactly `# END farhelm-centos-ci`. That is unverified: the comment at `:351-353` implies it was, and the history is
not in the reviewed export.

How to verify: in a scratch `HOME`, put
`# BEGIN farhelm-centos-ci farhelm-centos-target-abc123 (scripts/test-provision-centos.sh)` followed by some ordinary
stanzas in `~/.ssh/config`, and run the sweep part of the script. The stanzas disappear.

Fix sketch: before scrubbing, require that the end marker appears after the begin marker (for example, have awk exit
nonzero when it reaches the end of the file with `skip` still set), and refuse to publish, leaving the config untouched
with a message.

Suggested bucket: the reviewer said high, because the harm comes through maintainer tooling, and highest only if the
trigger is judged accidental. Recorded under highest: the loss lands on the user's own ssh configuration, outside the
tooling, and a partial hand deletion of a stray block is a plausible accident. Triage settles the bucket.

Confidence: confirmed for the mechanism by code reading. The trigger is likely rare; the premise that a begin line
without an end line arises without deliberate editing is unverified.

Reviewer's proposed drop, kept for triage: the trigger may count as deliberate setup. SPEC.md "Local authority and trust
between hosts": "Conditions the user ... deliberately set up outside Farhelm count as the user's intent: ... symlinks
and files they reshape by hand. If the trigger takes a deliberate setup and the consequence follows from what that setup
means, guarding against it is not Farhelm's job." The reviewer's counterpoint, which recording shares: deleting the
user's unrelated stanzas does not follow from what a half-deleted marker block means. Also
`review_feedback_queue/AGENTS.md`, "Priority buckets": "Harm confined to maintainer tooling ... stays out of `highest`
unless it is catastrophic."

Possible cover: SPEC.md "Local authority and trust between hosts"; `TRIAGE_OUTCOMES.md`
"ssh-stanza-removal-destroy-existing-configuration.md" and "legacy-ssh-stanza-cleanup-has-own-destructive.md", which
made the rewrites atomic and cover interruption only, not a missing end marker.
