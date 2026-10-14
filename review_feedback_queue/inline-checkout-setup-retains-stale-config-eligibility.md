# Clearing configuration leaves inline checkout setup hidden

Reviewed commit: 3d0bd43a7f9cf2f87adcb0077f011e9d5ddcb05d

## TLDR

If the checkout folder is cleared while a repository remains selected, the launcher shows a failed preview but does not
offer its new setup field.

## Details

Suggested bucket: `other`. Confidence: confirmed by static control-flow inspection; no browser reproduction. Source:
dedicated `gpt-6-astra` high adversarial review. Proposed drop: none.

crates/farhelm-ui/src/list/create_form.rs:3401–3422 retains a false needs-root observation using only host, connection
and installation identity. With a repository selected and its suggestions closed, that false observation disables
discovery. Clearing the effective root through the CLI or another client advances the configuration revision and
refreshes the preview (2775–2859), but never invalidates the observation. Setup therefore stays hidden at 3495 and 5322
despite the missing folder. Refocusing the repository field or reopening the launcher recovers. Invalidate setup
observations when the observed configuration revision changes, while preserving the optimization against rescanning on
ordinary title edits.

Possible cover: SPEC_impl.md:491–497 expressly describes retaining observations and returning to preview-only behavior.
It does not explicitly accept stale eligibility after a configuration change; SPEC.md:686–690 requires setup when the
selected host lacks a folder. No matching Planned item, BUGS entry, queue item or ledger decision found.

Status at frozen endpoint: Persists at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703: create_form.rs:3529–3550 and 3623
retain the same logic; configuration changes still refresh only preview authority at 2889–2966. Current SPEC.md:757–761
retains the requirement. No fixing commit identified.

Review coverage: full commit diff read: true. No commit-diff content remains unread. The 13,089-line triage ledger was
searched for relevant terms and matching sections were read, not read exhaustively. Unrelated queue finding bodies,
unrelated specification sections, and surrounding modules outside the stated ranges were not read. Aggregate outputs
truncated some auxiliary material; relevant code/specification content was reopened in smaller reads. No Sol reports or
new working-copy queue additions were read.

Runtime evidence: None. Static review only, as requested. Applied the supplied test-authoring checklist to changed tests
and fixtures. No checkout or scratch files created, modified or deleted; no fetch, rebase or helper agents.
