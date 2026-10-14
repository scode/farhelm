# Bare filenames with digit-leading extensions are not recognized

Reviewed commit: a5f5f8213eeb0a1a5161c22cf6260501aca81be9

## TLDR

Printed filenames such as backup.7z cannot be hovered or downloaded unless the program includes a directory prefix.

## Details

Suggested bucket: `other`. Confidence: confirmed by inspection of the recognizer. Source: dedicated `gpt-6-astra` high
adversarial review. Proposed drop: none.

crates/farhelm-ui/assets/terminal-files.js:24 requires a slash or an extension starting with [a-z]. Consequently
backup.7z and backup.001 fail recognition, although neither is a version number and both have file extensions.
./backup.7z works. SPEC.md:1363–1365 and website/src/content/docs/docs/using/work-in-a-session.mdx:53–55 promise
relative filenames with extensions without this restriction. Scope: plain terminal text; OSC file hyperlinks and
slash-containing paths are unaffected.

Possible cover: SPEC_impl.md:1611 calls recognition conservative, but does not specify excluding digit-leading
extensions. No matching Planned item, queue entry, BUGS entry, or ledger decision found.

Status at frozen endpoint: Persists at pinned main; pathTokens is unchanged.

Review coverage: full commit diff read: true. No reviewed-commit diff content remains unread. Unrelated portions of the
13,089-line triage ledger and unreferenced queue findings were not read exhaustively; coverage checks used targeted
searches and full relevant entries. The minified xterm JavaScript implementation was not independently audited, and the
later hover-fix test diff was not reviewed completely. No browser/native runtime reproduction was performed.

Runtime evidence: None, as requested by default. Source inspection only; no files created, modified or deleted, no
fetch/rebase, and no helper agents. Highest-category proposed drops remain pending the parent's independent drop review.
