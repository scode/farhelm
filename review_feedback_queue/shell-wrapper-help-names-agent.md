# Shell wrapper fields describe an agent command

Reviewed commit: 22054a595bc3d502b64af6040ec4b7355b3893da

## TLDR

A shell’s wrapper field says it runs before an agent command.

## Details

**other**. Confirmed by source and contract inspection.

confirmed: view.rs:1914–1946 unconditionally renders the wrapper row and its tooltip
(`Bash text before Farhelm's agent command`) and inline help
(`Runs through Bash before the agent command; blank means none.`). Templates/editor/rows.rs:282–285 likewise calls it
Farhelm's agent command while templates/fields.rs:217 retains the field for shell templates. The request retains the
same parsed prefix at request.rs:469–491. SPEC.md Concepts :68–69 and Creation :1033–1044 explicitly say the wrapper
applies to shells. Estimated bucket other (field works, but help misstates what is run). Possible_cover: none. Tip
8bcca67df1afe6eb0f184ef73b9952ed7ebb329f has no launcher or template editor edits after the landing.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `shell-launcher/f3`.
