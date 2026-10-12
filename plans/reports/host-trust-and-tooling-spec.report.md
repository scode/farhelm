## What this was about

Twelve triaged findings described cases where Farhelm or its development tools do not defend against a hostile or broken
host, outside changes to managed checkouts, overlapping tooling, or operator-caused setup races. On 2026-10-11 you
accepted those limits and asked for the specifications to record them. This plan carries out all twelve outcomes in one
documentation PR. It changes no executable behavior.

## Things you should know

A malicious or arbitrarily broken host may deny service to the helm or desktop app, even where Farhelm could avoid it.
The prohibition against corrupting the helm's state or affecting other hosts' security or data remains. Existing spacing
of session-list refreshes remains, so a flood of a supervisor's "sessions changed" hints causes no more refresh work
than a busy honest host. Slowdowns or memory exhaustion inside the bundled terminal library still call for upgrading
that library when upstream fixes the problem. Review found an older promise that a broken remote filesystem could never
interrupt helm availability; an explicit cross-reference now makes it subject to this accepted limit. Long management
operations on healthy hosts still must not block other sessions' terminal input or output.

Farhelm has dedicated ownership of the managed checkouts it creates, their checkout roots and trash. Snapshot restores,
user-arranged kernel overrides, uncommon filesystems such as FAT, and replacement during the ownership check's brief
check-then-move window are unsupported. Ordinary folder replacement and supported filesystem remounts remain covered.
Installation, state-directory, symlink and canonical-path protections are unchanged. Development tooling now expressly
requires one command at a time per checkout; concurrent agents use separate checkouts.

For Farhelm commands that publish consecutive atomic file updates, interruption between them may leave a temporary
mismatch that rerunning the same command repairs. For example, killing setup between its service-definition writes may
leave the helm and supervisor configured for different state directories; rerunning setup is the accepted recovery. The
rule is general rather than limited to service definitions, and accepts only mismatches that the same command repairs.
Switching a remote host to non-panel setup while a panel setup or Update is queued or running remains best effort; a
visible failure is sufficient. Uninstall preview reports ownership, recognition and ambiguity blockers, but does not
predict refusals from currently held locks. Its existing warning names a running install or upgrade; the real uninstall
run clearly refuses any held lock. No warning text was changed.

All twelve feedback files and their index entries were removed, and their triage records identify the completed change
and PR. No outcome tripped the complexity gate. No changelog fragment is needed for documenting accepted existing
behavior.

## Open questions and possible follow-ups

None. No outcome was dropped or left waiting on a maintainer decision.

## The PRs

- [#1923](https://github.com/scode/farhelm/pull/1923/changes) — records the accepted host, managed-checkout, tooling and
  setup limits; one draft PR for all twelve outcomes.

## Checks run, reused and skipped

- Run: targeted `dprint check` on SPEC.md, SPEC_impl.md, TRIAGE_OUTCOMES.md and the feedback index. All passed.
- Run: the private bookkeeping helper's format/type checks and four parsing/refusal tests. All passed; the complete diff
  was also checked for the twelve record updates and matching file/index removals.
- Reused: source-review coverage of unchanged outcome records and removals after adding the availability
  cross-reference. The rebase brought in other executors' queue claims and a disjoint cleanup of 21 feedback findings,
  inspected in full. A mechanical index conflict was resolved by preserving both sets of removals; all 85 remaining
  findings are indexed exactly once. Final diff inspection also caught and corrected the private helper dropping a
  category heading when removing its last entry. No product code or specified behavior changed during the rebase.
- Skipped: Rust, JavaScript, desktop, installer, provisioning and browser runtime tests. This is ordinary Markdown with
  no generated artifact or executable example affected. The website does not restate any changed rule incorrectly, so no
  website edit or build was needed.

## The review gate's outcome

The required source review found one conflicting broken-filesystem availability promise. The cross-reference resolved
it; the follow-up found no further issue and confirmed the intended security, data and ownership boundaries. The
required commit/PR wording cold read and the report cold read passed before delivery.
