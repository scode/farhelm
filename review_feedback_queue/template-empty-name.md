# Editing a template silently drops its instruction to clear the name

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Editing a template silently drops its instruction to clear the name.

## Rebase context

At rebase onto `eb71c32e0f14f4569938e7da293e366aa07eae8a`, the shipped agent CLI can also create the explicit empty
session-name value: `farhelm agent template create/edit … --title ''` forwards it unchanged
(`crates/farhelm/src/main.rs:776–807`; `crates/farhelm-helm/src/agent_requests.rs:1804–1837`). The GUI still omits blank
session-name text when saving (`crates/farhelm-ui/src/list/templates.rs:167`). The API/import-only authoring caveat
below no longer describes every way to reach this finding.

## Details

`F51 / COR-TEMPLATE-EMPTY-NAME` — **definite** — `crates/farhelm-ui/src/list/templates.rs:167` — Editing a template
silently drops its instruction to clear the name

Editing a saved template can silently remove its instruction to clear the launcher's session name. Here “name” means the
session name the template applies, not the template's own identifying name. The API accepts an explicitly empty
session-name field, which clears a name supplied by an earlier template or launcher edit. The editor displays that
stored empty string as the same blank input it uses for “leave as is.” On save, it omits blank text rather than
preserving the explicit empty value.

An unrelated edit and save can therefore change the stored template from “clear the session name” to “keep the existing
session name.” When templates are stacked, the earlier name survives instead of being cleared. Give the session-name
field explicit leave, set, and clear/reset states, and preserve the empty value through editing, saving, and
application.

Suggested bucket: highest

Possible cover: none

Caveats: This requires a template with an explicitly empty session-name field created through another API client or an
import; the current GUI cannot author that state. The loss concerns stored template instructions and session-name
metadata, not session deletion. No runtime reproduction was performed.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **definite**. Suggested queue bucket: **highest**.

Originating reviewers and passes: `ui_data p2`.

Possible cover recorded during collection: none identified.

Collection caveats: Requires otherAPI/imported explicitempty; GUIcannotauthor;metadata only, no sessiondeletion; no
repro.

## Filed reviewer metadata

- `ui_data p2`: confidence as filed: **definite / confirmed**. Premise: a template containing `{"name":""}` is valid
  stored data; the shared shape check at `crates/farhelm-proto/src/launcher.rs:280–302` accepts it, and template
  application at 606–607 treats it as an explicit value. Suggested bucket as filed: **highest**, because saving loses
  stored template data.
