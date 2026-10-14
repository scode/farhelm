# Legacy command edits can erase newlines before save validation sees them

Independent drop decision: a fresh `gpt-6.1-sol` high reviewer rejected the coverage exclusion. The chosen refusal at
save does not clearly accept unnoticed sanitization that bypasses that refusal. This one finding covers both launch and
resume commands; actual browser editing and persistence remain unverified.

Reviewed commit: dd9573cc07782949917597520bdc49ca84706cc0

## TLDR

Editing a legacy multiline template command can silently change its argument text and then save successfully.

## Details

Suggested bucket: `highest`. Confidence: likely; actual browser edit and persistence were not reproduced. Source:
dedicated `gpt-6.1-sol` high adversarial review. Proposed drop: Proposed covered by TRIAGE_OUTCOMES.md headings
template-command-editors-hide-meaningful-characters-silently-saved-command-editor.md and
template-command-editors-hide-meaningful-characters-silently-saved-resume-command-editor.md: the maintainer chose
reject-at-save and explicitly rejected lossless escaped editing. Independent drop review required. Coverage is ambiguous
because the resulting sanitized draft passes validation without an explicit repair acknowledgement..

crates/farhelm-ui/src/list/templates.rs:644–645 and 653–654 still seed raw command text into single-line inputs and
replace the whole draft from the input event. For a legacy command such as "echo 'first\nsecond'", HTML input newline
sanitization means an ordinary edit can produce "echo 'firstsecond!'". check_template_shape at
farhelm-proto/src/launcher.rs:339–354 validates the resulting draft, whose newline has already disappeared;
templates.rs:945–955 therefore cannot detect that loss. Applies independently to launch and resume commands. Opening and
saving without an input event preserves the raw draft and is refused. This behavior predates the commit, which removes
both corresponding feedback files. The browser sanitization premise was not reproduced here.

Possible cover: Reviewed SPEC.md:219–222 explicitly permits legacy templates to launch and requires repair before saving
changes. The ledger chooses single-line editing plus save refusal. Later ledger headings
template-resume-command-editor-has-same-unsafe-display.md and
template-command-editor-displays-untrusted-command-bytes.md treat legacy raw display as accepted. Those decisions
clearly cover raw legacy presentation; whether they also accept unnoticed newline removal during an edit needs
adjudication.

Status at frozen endpoint: Persists at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703: templates.rs:655–667 retains raw
single-line controls and whole-value input replacement; launcher.rs:347–363 validates only the resulting values. No
fixing commit identified.

Review coverage: full commit diff read: true. No commit diff content remains unread. Full unchanged modules, complete
specifications and the 13,089-line triage ledger were not read wholesale; relevant sections and searches were used. An
initial broad search was truncated and was not treated as complete ledger coverage; pertinent sections were subsequently
opened. Genuine OS clipboard/native desktop behavior, actual legacy browser edits and injected fixture failures were not
exercised. No checkout files were created, modified or deleted.

Runtime evidence: None, as requested. Source inspection only; no reproduction commands or test-process environment
changes.
