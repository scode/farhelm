# Save and Close can dismiss a confirmation opened after the close answer

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

A successful Save and Close can dismiss a newer unanswered tunnel-permission question.

## Details

The close prompt closes before its answer starts the batch (crates/gui/farhelm-gui/src/prompt.rs:829–837.
Settings/editing.rs:613–624). The Settings page and navigation remain interactive (settings.rs:1588–1606). The batch can
start or join an ordinary credential save (settings/editing.rs:175–198, :355–372). It holds a credential key
(settings/credentials/editor.rs:302–324), so the Mac's tunnel switch can still open its warning: the switch checks only
the preference key (settings.rs:1221–1239, :1271–1305). The consent route opens a child prompt
(terminal/tunnels.rs:682–717), and raising it over approvals preserves its Settings parent (layers.rs:2197–2217). On a
normal successful credential save the editor departs and reports success (settings/credentials/editor.rs:310–323.
Settings/credentials.rs:389–405). With no edits remaining, the batch calls finish_close without checking subsequent
interaction (settings/editing.rs:453–465, :546–549). The layer manager removes Settings and the newer warning together
(layers.rs:1298–1317, :1479–1502). SPEC_impl.md:1076–1080 requires an asynchronous completion that closes something to
ask the focus coordinator whether the user has moved on. Preserve newer interaction before closing the Settings branch,
for example by treating a newer child question as superseding automatic closure. The original orphan hypothesis is
refuted by descendant cleanup. The confirmed effect is dismissal of the newer question. There is no additional write or
lost save.

Confidence: confirmed by inspection for the Mac path with tunnels off, no competing preferences write, and a successful
credential save without delivery warnings.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `footer/l15`.
