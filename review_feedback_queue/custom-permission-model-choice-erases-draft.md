# Choosing a model erases unfinished custom permission flags

Reviewed commit: a3ff0b6b617cad499ab4b129ef4282861b70436b

## TLDR

Choosing a model can erase custom permission flags you were still editing, even when the agent type stays the same.

## Details

Suggested bucket: `highest`, because the erased text is an unsaved user draft. Confidence: confirmed by static data-flow
tracing; no browser reproduction. Source: separate adversarial trailing-review agent, permissions scope. Proposed drop:
none. Possible cover: none found.

Against the reviewed commit, `crates/farhelm-ui/src/list/create_form.rs:771–775` parses the raw flags before a model
search choice, then `805–811` unconditionally rebuilds the editor from the parsed selection.
`crates/farhelm-ui/src/launch_composer.rs:174–180` returns `None` for unfinished quotes, so the reconstruction replaces
the text with an empty string. Re-selecting the current agent follows the same path at `create_form.rs:729–762` and
`4003–4053`. The mechanism remains at the pinned trailing-review tip, `507dbcbbcfa00173ca7e4d24c79f3990025963b6`.

Select Claude and custom permissions, type `--permission-mode 'ask me` with its unfinished quote, then choose a Claude
model through composer search. The flags disappear despite no permission reset or agent change. The dedicated model
picker avoids this reconstruction, so the two ways of choosing a model differ.

Keep raw editor text when reconciliation leaves its owning agent and permission mode unchanged. Reseeding from parsed
words belongs to an actual source replacement. A regression should exercise malformed flags followed by a same-agent
model search choice and current-agent re-selection. Completed flags retain their argument meaning; this finding is about
unfinished text. `SPEC_impl.md:2639–2640` at the reviewed commit explicitly preserves malformed quotes as repairable
editable text. No matching Planned item, accepted bug, queue item, ledger decision or filter was found.
