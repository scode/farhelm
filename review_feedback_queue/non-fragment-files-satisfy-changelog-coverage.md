# Non-fragment files satisfy changelog coverage

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Changelog coverage accepts files that fragment discovery never loads.

## Details

F336 — **definite** — `releasing/check-changelog.py:470–480` — Non-fragment files satisfy changelog coverage

The history sweep counts added or modified non-README files under the fragment directory, while discovery loads only
immediate Markdown children. A text file or nested Markdown can therefore satisfy coverage and format checks without
supplying the required release-note entry. Share one fragment-eligibility rule between discovery and history accounting,
including rename destinations.

## Evidence and triage context

- releasing/check-changelog.py:378–383 selects immediate *.md files.
- releasing/check-changelog.py:470–480 counts history paths; :812–839 reports sweep coverage.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- None found by the originating reviewer.

Caveats:

- No reproduction or currently malformed fragment alleged. False success is outside the rare safe-failure filter.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `automation_website_07_cor:p2:F1`.

- `automation_website_07_cor:p2:F1`: confidence as filed: definite; suggested bucket as filed: other.
