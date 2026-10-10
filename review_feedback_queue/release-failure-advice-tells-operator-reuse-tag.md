# Release failure advice tells the operator to reuse a tag

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Release recovery advice could lead an operator to reuse an immutable tag.

## Details

F66 — **possible** — `.github/dist-build-setup.yml:96` — Release failure advice tells the operator to reuse a tag

The release refusal emits advice to delete and reuse a tag, contradicting the required immutable-tag recovery procedure.
Following that instruction could remove the original reference identifying the failed attempt. The workflow itself does
not delete or move tags, and no operator-induced loss was observed. Change the emitted advice to preserve the tag and
use the appropriate new-version procedure, then regenerate the workflow. Nearby stale comments are accompanying
documentation-only cleanup.

## Evidence and triage context

- .github/dist-build-setup.yml:93-97: a tag/version mismatch prints 'delete and re-push the tag', then exits 1.
- .github/workflows/release.yml:142-146: the generated, executable workflow contains the same message.
- releasing/AGENTS.md:287-291: failed builds require a new patch version because tag names are never reused.
- releasing/AGENTS.md:371-385: failed RC tags remain; subsequent RC/dev attempts use new versions.
- releasing/AGENTS.md:512-515: incomplete-release recovery deletes only the GitHub release, never its tag.
- .github/dist-build-setup.yml:93-97 and generated .github/workflows/release.yml:142-146 emit the instruction.
  releasing/AGENTS.md:287-291, :371-385 and :512-515 require preserving tags. review_feedback_queue/FILTER.md:28-42
  requires the whole consequence to fit and excludes loss of user-owned work. No automated tag mutation or observed
  operator loss is claimed.

Retained confidence: **possible**. Suggested bucket: **highest**.

Possible cover:

- review_feedback_queue/FILTER.md:24-42, 'Rare, self-correcting glitches and imprecise diagnostics', would cover
  misleading diagnostics alone. Its whole-consequence requirement and user-owned-work exclusion prevent a confident
  match when the emitted recovery advice can cause loss of the original tag reference.

Caveats:

- No workflow operation deletes or moves a tag.
- No operator following this advice, lost commit, or damaged release was observed.
- The possible highest classification concerns release provenance lost through following the instruction, not automatic
  destruction.
- TODO.md:33-43, BUGS.md, the queue index, and targeted ledger searches provided no matching accepted disposition.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `automation_website_01_cor:p1:F1`,
`automation_website_01_sec:p1:C2`.

- `automation_website_01_cor:p1:F1`: confidence as filed: definite; suggested bucket as filed: highest.
- `automation_website_01_sec:p1:C2`: confidence as filed: not separately tagged in candidate list; suggested bucket as
  filed: not separately tagged in candidate list.
