# Bash source admission retains literal-splitter restrictions

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

Some valid Bash command source is refused despite the product's Bash-source promise.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The retained literal splitter rejects supported source constructs before Bash receives them. The exact stored-command
reinterpretation decision does not explicitly settle this admission contract conflict. Placeholder certification and
pre-Stop refusal checks remain independently source-refuted for their named cases. No command or example payload was
executed.

Source locations:
`SPEC.md:754-758; crates/farhelm-proto/src/session_launch.rs:142,164-166,504-505; crates/farhelm-proto/src/command_shell.rs:270-283,373,407-408`.
Historical revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Synthesis preserves and narrows f1. This is valid-source rejection at actual launch admission, not the separate g183
invalid-template save defect. Native REST create resolves LaunchRequest::Command via command.validate at
farhelm-helm/src/sessions.rs:2286-2297. CLI/template launch resolution constructs CommandLaunch from applied source and
uses that same path at agent_launch.rs:234-256. The supervisor independently invokes validate_new at
service/core.rs:8337-8339. Therefore literal validation rejects a valid marker-free heredoc body containing an unmatched
apostrophe, and native placement validation rejects real unquoted Bash argument words adjacent to punctuation (printf
'%s' {cwd}; and echo $(agent {cwd})). SPEC.md:754-758 promises full Bash source and permits unquoted whole-word
placeholders; SPEC_impl.md:1904-1907 explicitly retains the literal split and exact literal slots, so retain
possible_cover as a conflicting narrower implementation contract rather than silently resolving the conflict. Template
storage is different: templates.rs:98-109 calls only check_template_shape; launcher.rs:463-525 checks
bounds/presentation/wrapper shape but no command Bash placement. UI Save-as-template at save_template.rs:226-253 and
template editor at templates.rs:1985-2039 use that shape plus editor compatibility checks; templates.rs:862-909,965-975
adds no command placement check. Thus single-line valid printf '%s' {cwd}; can save as a template but later fails native
launch; quoted-invalid printf '%s' '{cwd}' can also save and later fails, the latter being g183's separate finding.
Multiline template saves are deliberately shape-limited, so the heredoc example is limited to direct command requests,
not a template-save complaint. Suggested bucket other; confidence confirmed source/spec discrepancy; proposed_drop null;
possible_cover SPEC_impl.md:1904-1907 and landed command-shell-meaning plan:69,77-79. Reviewed
d6645f771bee8ed03a66486ef8755834937d759d; assigned introducing commit 6862807cc3ff58e530ec5145f8b067db8af87b88. No
runtime actions performed.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g182/f1`, `a012/f6`. These are original
reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain
in the private review evidence. No fix or triage decision was executed.
