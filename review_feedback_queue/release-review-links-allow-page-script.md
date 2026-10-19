# Release-review Markdown links can inject page script

Reviewed commit: c2c0334aabcc7fa7ca9e2e7f668c49851e54058c

## TLDR

A hostile link in the draft notes used to generate a release-review page can inject page script. The draft is curated
maintainer input; exposure to an attacker and consequential harm have not been established.

## Details

The link formatter escapes with `html.escape(..., quote=False)` and then interpolates the destination into a quoted
`href` attribute (`releasing/notes-review-page.py:72–85`). A destination containing `" onmouseover="document.title=1`
introduces an event attribute. The formatter also accepts `javascript:` destinations. Attribute quoting and allowed URL
schemes need separate checks; escaping text alone does not establish a safe link. Inspect the generated markup for those
inputs, then verify that arbitrary attributes and executable schemes are refused or rendered as inert text. No browser
exploitation was attempted during review.

Suggested bucket: **other**, for maintainer tooling without an established catastrophic or external security
consequence. Confidence: **confirmed unsafe attribute construction**; adversarial input exposure and meaningful harm are
unverified. Provenance: same-model adversarial trailing review, followed by an independent fresh-context exclusion
audit. Proposed drop: none after audit.

Possible cover: the script's documentation (`4–8,27–34`) and the changed stable-release procedure describe an
agent-authored, curated scratch draft published through the snippets workflow. The initial reviewer proposed excluding
the issue because it did not establish an untrusted-input boundary; a malicious contributor able to land executable
repository changes does not alone establish one. The independent auditor found that insufficient for a drop: the unsafe
sink is real, and no explicit specification, decision or filter accepts it. No matching Planned item, known-bug
acceptance, existing queue item or ledger decision was found. Triage should assess the intended input trust boundary
before treating this as an exploitable vulnerability.
