### What this was about

The SSH connection help offers commands to copy into a terminal. Two review findings concerned unusual characters in the
user's configured destination or state path: a copied command could be interpreted differently by fish, or hidden
terminal controls could act differently from their displayed escapes. The maintainer decided that Farhelm trusts the
user's own configuration rather than adding defenses for those values.

The specification now records that decision for configured host names, SSH destinations and the helm's state directory
path. Each finding has its own PR removing it from the feedback queue and recording its execution in the triage ledger.

### Things you should know

This changes the specification and tracking only. The copied commands and clipboard behavior are unchanged. Unusual
shell- or terminal-interpreted characters in those locally configured values remain an accepted risk. The maintainer's
answer that this also covers the helm's state path is preserved in the first outcome's execution record.

The trust applies only to those user-configured values. Text supplied by a remote host remains untrusted and must still
be escaped outside a terminal. Inspection confirmed that these SSH commands embed the user's destination and the helm's
own state path, with its working directory used to preserve relative state-path resolution; no embedded value came from
a remote supervisor.

### Open questions and possible follow-ups

None identified within the agreed scope.

### PRs

- [#2040](https://github.com/scode/farhelm/pull/2040/changes): trust rule and closure of the fish-quoting finding; draft
  on main.
- [#2042](https://github.com/scode/farhelm/pull/2042/changes): closure of the covered hidden-control-character finding;
  draft on #2040's bookmark.

### Checks run, reused and skipped

`dprint check` passed for SPEC.md, TRIAGE_OUTCOMES.md and the feedback index; the diff whitespace check passed. Both
feedback files and their index entries are removed, and both execution records name their own change, bookmark and PR.
No code or test files changed, and neither PR touches `plans/`.

The only main change during execution was another plan's queue claim. Its full diff was inspected and the first PR was
rebased without changing the reviewed content. No runtime evidence was reused. Rust, JavaScript, browser, desktop and
website builds or tests were skipped under the documentation-only rule. No exploit reproduction was attempted; this
implements the maintainer's accepted trust policy rather than changing quoting or clipboard behavior.

### Review gate

Each PR passed its required fresh-context gpt-6.1-sol source review at high effort, covering correctness, design, scope
and idiomatic prose. No findings. Commit and PR wording passed separate cold reads. The documentation pass covered the
specification paragraph and both ledger updates. Both PRs remain drafts; the executor has marked neither ready nor
merged either one.

### Landing

Landed as #2040 and #2042, second of the round's three spec-only plans, after triage-power-loss-spec (#2039, #2043) and
before triage-hand-edited-units-spec (#2041, #2049). The hand-edit plan adds its own exception to the 2026-10-10 refusal
rule right below this plan's paragraph; the two read together without conflict.

#### Review

An independent fresh-context review found nothing blocking. It checked the code: the SSH help's copied commands are
built only from the stored or typed destination, the helm's own state directory, the helm's working directory when that
path is relative, and fixed options. Stored destinations are written only by host registration, the change-destination
action and `--ensure-hosts`; no remote supervisor can set them. The session header's copy actions, which do carry remote
text, stay outside the new trust, and the escaping rule for remote text is unchanged.

#### Fix made while landing

- The paragraph said only that the commands "need not defend against" unusual characters, while the 2026-10-10 rule
  right after it requires unsupported configurations to be refused with a clear message. It now says Farhelm need
  neither defend against such values nor detect or refuse them, so a later reviewer cannot file "accepted without
  refusal" under the refusal rule.
- It now names the working directory a relative state path resolves against, which the copied command also contains.

#### Checks

- Run now: `dprint check` on SPEC.md after the fix, and on main after all three plans landed.
- Not run: anything else; only Markdown changed.
