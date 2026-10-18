# The template name editor conceals formatting characters

Reviewed commit: 5e1f1cfffe38f41e450ab70d440ca8fabac8be36

## TLDR

An agent-authored template name is escaped in the template list and approval card but displayed raw when opened for
editing, concealing invisible or direction-changing characters.

## Details

`crates/farhelm-ui/src/list/templates.rs:1669` binds `current.name` directly. Names containing U+200B or U+202E pass the
ingress checks because those check `char::is_control` (`crates/farhelm-supervisor/src/service/handlers.rs:3396–3425`;
`crates/farhelm-proto/src/launcher.rs:371–390`). Helm storage retains the name unchanged
(`crates/farhelm-helm/src/store.rs:3957–4064`). SPEC.md:2837–2842 requires visible escaping of host-originated GUI text
outside terminals. Give this name editor a safe presentation while preserving original bytes until an explicit edit.

Untouched Save does not silently sanitize values: the draft holds raw wire values, escaped read-only controls are
presentation only, and Save serializes `draft()` (`templates.rs:1335–1358`). Unsafe model and folder values also survive
unrelated saves unchanged. The top-level template name lacks safe display; this finding does not allege save-time
corruption. Prohibited legacy command/session-name values fail validation instead of being sanitized.

Suggested bucket: **other**, for the demonstrated display defect. Confidence: **confirmed statically** through ingress,
storage, display and save. No browser reproduction, harmful wrong-target action or authorization bypass is established.
Agent template writes require approval, whose card already escapes the name.

Possible cover: `template-editor-conceals-peer-model-and-folder.md` in TRIAGE_OUTCOMES.md fixes model/folder fields;
`template-supplied-session-names-rendered-raw.md` fixes a template's session-name field. Neither is the template's own
catalog name. The model-summary and command-editor decisions likewise concern other fields. No matching Planned, BUGS or
current queue item was found.

Proposed drop: none. Provenance: persistent Astra/high review, `ui-templates-and-trash-1/f1`.
