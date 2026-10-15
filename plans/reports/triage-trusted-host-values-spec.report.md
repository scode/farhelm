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
