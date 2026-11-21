# A queued file read can change the credential submitted by an open Settings close question

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

A file read can change the Codex login draft while the Save and Close question is open, leaving the chosen file
unreviewed before it is saved.

## Details

A saved Codex login can have replacement text and a pending selected-file read. In
crates/gui/farhelm-gui/src/settings/credentials/editor.rs:396–445 the picker callback yields to a separate background
read (:410–421), then switches the active secret to the completed file if the paste revision is unchanged (:424–441).
Opening the Settings question does not cancel that read or capture a draft revision
(crates/gui/farhelm-gui/src/settings/editing.rs:559–625). The awaited read can therefore finish behind the centered
question. On Save, editing.rs:342–373, :471–512, and :197–198 dispatch the live editor, whose :289–307 reads
secret_value() from the new chosen file (:191–197). The native picker returns a path from its completion independently
of Farhelm's later file read (vendor/gpui-pre-macos/src/platform.rs:892–919.
Vendor/gpui-pre-linux/src/linux/platform.rs:862–874). SPEC_impl.md:973–1000 describes presenting frozen confirmation
state, and :5976–5981 separates presenting a staged file's name and validation from saving it. Bind Close → Save to the
shown credential draft revision and return to the editor if the file source changes, so the file can be reviewed and
saved explicitly.

Confidence: likely with the named premise that the native picker has returned and its separate background read completes
after a dirty credential opens the close question and before Save is answered.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `footerTests/l4`.
