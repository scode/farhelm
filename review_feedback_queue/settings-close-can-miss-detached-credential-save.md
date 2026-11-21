# Settings close can lose track of a save already admitted by a dismissed editor

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

Settings can close without waiting for a credential save whose editor is already closed.

## Details

The global Settings close flow finds credential saves only through the mounted editor or a handle already owned by its
own batch (`crates/gui/farhelm-gui/src/settings/editing.rs:255–266`). A direct credential Save can survive removal of
its editor via Back or Cancel because the admitted request is owned by the operation store
(`settings/credentials/editor.rs:302–324,507–530`, `settings/credentials.rs:389–405`, `settings/drafts.rs:157–172`). If
another draft triggers the close question before that save settles, Settings can offer Discard or close without joining
the admitted save (`settings/editing.rs:342–372,559–609`). Track admitted saves independently of mounted editors so the
close flow observes their receipts. Direct editor tests settle the file selection before saving and do not check an
admitted save after leaving its editor (`settings/credential_tests.rs:588–631,667–740`).

Confidence: confirmed by inspection.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `footer/l10`, `footerTests/l3`.
