# Open checkout settings can overwrite a newer command-line edit

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

Saving the post-clone command from a Settings window that was already open can erase newer edits you made from the
command line. Using the old view’s Use default can also clear a newer host-folder override.

## Details

Bucket: **highest**. GUI migration final whole-tree review, R8-5.

Source references: `crates/farhelm/src/main.rs:2611`,
`crates/farhelm-helm/src/checkout_config.rs:1085, 1105–1111, 1360–1395`,
`crates/gui/farhelm-gui/src/settings/checkout_field.rs:198–216, 293–338, 350–410`.

Checkout Settings reads the field at open and on its own completion or retry paths. The CLI can update the same database
directly. Saving the shown command or clearing the host override sends a replacement without its prior value or
revision. The writer compares requested and current values for no-op detection, but its revision counter only
invalidates reads. The next checkout uses the overwritten hook or inherited folder.

Sol rated this source-confirmed and critical. Opus verification R8-5 confirmed the gap and rated it noncritical because
it requires changing the setting from the CLI while the dialog is open and then saving the stale editor. Highest
reflects an established loss of user-authored saved command text, which can be a multi-line hook. The folder-override
variant alone is a recoverable configuration error. SPEC.md “One GUI at a time” protects concurrent CLI writes and names
templates and number-key bindings as the exceptions.

Send the read's baseline revision or value and compare it in the setter's write transaction. Test a CLI-equivalent write
between the GUI read and save, including a stale Use default.
