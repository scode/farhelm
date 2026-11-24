# Trailing code review checkpoint

Last reviewed commit: `1b3fe49af86979aebc2719965682a17f8c6366ac`

This is the exclusive starting point for the next trailing code review of `main`. Advance it only after completing the
procedure in `review_feedback_queue/.agents/trailing-review.md`, together with any resulting queue updates. A proposed
checkpoint reaches `main` only when its review-result PR is merged.

This file holds only the checkpoint and the latest run's completion record. Earlier runs' records are in
`review_feedback_queue/TRAILING_REVIEW_HISTORY.md`.

## Review through 1b3fe49a

The range starts after `66ad3aaa95ef03a2130e2d0194d58353912ae9da` and includes
`1b3fe49af86979aebc2719965682a17f8c6366ac`. Of the 185 commits inventoried, 68 have in-scope patches with 25,307
additions and 7,099 deletions across individual commits, touching 321 unique paths. The other 117 contain only excluded
material, including plans, review bookkeeping, TODO and flake records, agent instructions, historical records, ordinary
documentation, and release prose.

Antigravity `agy` (`gemini-4-neon` at high reasoning effort) reviewed 64 functional groups over 200 turns. The groups
covered macOS service stop grace and brick-test coordinator, guest workflow, remote-Mac uninstall, and test suites;
maintainer drift-upgrade, private-config, and test-worker tooling; Claude trust home-directory policy and release notes;
Tensorlake sandbox identity, leftover discovery, and Stop-and-suspend feedback; removing the retired
`Agent's own setting` permission mode and guarded plain-Replace; Codex recipe credentials across helm, supervisor, and
specs; stale launcher choices and approval-card wording; Wayland clipboard file paste; toolkit reduced motion and Trash
animation; remembered-tab fallback and Muse conversation reporting; repository-cache lock redesign; removing the host
sign-in route lock; GUI button restyling and release golden gates; pending-upgrade sidebar reminders; host automatic
updates after 2FA sign-in; registered custom agents end to end across protocol, supervisor, CLI, helm, e2e tests,
Settings, launcher, and templates; positional hook attribution for Grok, Codex, and shared corridors; and OSC 8 terminal
link disclosure. Delivery audits matched all 57,876 assigned patch lines. Every group completed later-turn synthesis and
a final discovery pass with no new findings, and the run ended quiescent with no open leads or pending cross-group
input.

The reviewers logged 172 active findings, 0 withdrawn findings (1 withdrawn candidate superseded by a verified finding),
and 334 open candidates before deduplication and independent checks. Consolidating all 506 open records produced 133
verified defect groups, 2 unverified-after-investigation groups, 6 rejected groups (5 reviewer-proposed spec-accepted
drops and 1 filter drop), and 333 excluded candidate groups. Two fresh-context `agy` (`gemini-4-neon`, high)
consultations checked all 6 entries in the independent drop register, agreeing with all 6 and noting no unrelated leads.
The resulting queue adds 130 items (5 `highest`, 15 `high`, and 110 `other`; 2 additional `other` findings were
deduplicated against fixes already landed post-tip in `#4752`, and `trailing-cli-effort-hyphen-and-spec-drift.md` was
narrowed to what remains open after `#4752`) and extends 3 existing items
(`trailing-template-summary-misstates-unsupported-permissions.md`,
`trailing-ssh-route-lock-leaks-cache-locks-on-delete-race.md`, and
`trailing-hosts-menu-rerun-reads-live-host-state.md`). Source inspection supports 128 new items and all 3 extensions; 2
new items preserve unverified premises after investigation. The review inventory, delivery audits, findings logs,
disposition log, and independent verdicts are retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca`.
