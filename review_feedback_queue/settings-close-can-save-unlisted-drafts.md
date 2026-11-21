# Save and Close can write a form omitted from its confirmation

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

Save and Close can write an unlisted form and clear the saved alert token outside the user’s confirmation.

## Details

The close prompt displays the current dirty forms. Its answer captures no approved form identities
(`crates/gui/farhelm-gui/src/settings/editing.rs:559–589`). If another save is underway, the queued close answer
enumerates the live forms again after that save settles (`settings/editing.rs:342–367,453–465,617–624`). The window
still permits editing other forms during the wait. A later Notifications draft can therefore join a close operation
whose question omitted it. The existing server-change contract clears the saved access token when the new draft changes
the origin without supplying a replacement (`SPEC.md:2554–2557`, `SPEC_impl.md:5878–5887`). A late login-file read can
also create a credential draft the question did not list, conditional on file-selection completion timing. Capture the
approved form identities with the confirmation and carry them through deferred dispatch. Check each approved draft
revision independently of its form identity.

The same deferred admission can write an unlisted Terminal file-type policy. The saved preference controls which
session-file types may later be opened in a local app (`settings/file_types.rs:207–229`,
`crates/farhelm-helm/src/downloads.rs:204–220`). The file still requires a separate open gesture. Another variant
re-exposes a previously omitted credential or recipe draft after a Delete refusal
(`settings/credentials/editor.rs:269–276,369–374`, `settings/recipes/editor/leaving.rs:99–101,282–304`). The close
question never acquires the new form. A credential replacement through this branch remains conditional on a supported
overlapping recipe save changing the reference before Delete admission. The successful close case should assert both the
presented list and the eventual request bodies. The existing close failure test refuses its first write
(`settings/tests.rs:3258–3295`).

The Discard answer also acts after the shown form set can change (`settings/editing.rs:580–623`). If an in-flight
login-file read stages an additional draft that was absent from the question (`settings/credentials/editor.rs:398–444`),
closing through that answer can silently drop the new draft. The lost input is recoverable and distinct from overwriting
a stored secret.

Confidence: confirmed by inspection for the deferred-target route. The Codex route remains likely with a valid selected
login and a background read still pending after the native picker returns control.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `footer/l1`, `templateSave/l5`, `nav/l3`,
`footerTests/l1`, `footerTests/t1`, `footerTests/t4`, `footerEditors/l2`, `cards/l4`, `listCards/l5`, `recipeCards/l2`,
`upgradeSettings/l5`.
