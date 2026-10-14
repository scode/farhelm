# Update installation replaces the focused version button

Reviewed commit: 33b4f1507dff5d1b12e8f243cc4af17543f0a80a

## TLDR

After a keyboard user checks for updates and an update installs, the version control loses focus instead of carrying it
into the restart menu.

## Details

Suggested bucket: `other`. Confidence: Likely. Conditional replacement and missing transfer are confirmed; actual
shipped macOS WebKit focus behavior was not reproduced. Source: dedicated `gpt-6.1-sol` high adversarial review of this
commit. Proposed drop: none.

The check button exists only for `ReadoutControl::Check` (`crates/farhelm-ui/src/app_bar.rs:625–629`). An installed
update selects `UpdateMenu` (`crates/farhelm-ui/src/app_updater.rs:78`), removing that button and revealing a separately
mounted toggle (`app_bar.rs:604–623`). Neither branch transfers focus. The menu's focus restoration (`303–347`) only
covers an already open menu.

Keyboard-focus and activate the version button, retaining focus while installation finishes. Its focused DOM node
disappears; the restart toggle receives no explicit focus. This affects installed desktop builds. Retain one control or
transfer focus only if the disappearing control owned it.

Possible cover: None found. Installation and updates specifies the action change without explicitly accepting lost
focus.

Current-main status: persists by inspection at `4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703`; no fixing commit found. The
reviewer read the full commit diff and relevant traces; this is not an exhaustive review of unchanged surrounding code.
