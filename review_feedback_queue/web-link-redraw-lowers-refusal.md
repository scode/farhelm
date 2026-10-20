# A web-link redraw can hide its refusal explanation

Reviewed commit: b58e9a416ff3e0a6c6bdf4ec5fe7fea54c5adc99

## TLDR

After Farhelm refuses a web link whose destination is hidden by a tunnel-error notice, terminal output can hide the
explanation again while the pointer stays on the link. Further clicks remain refused; this does not bypass link
admission.

## Details

`crates/farhelm-ui/assets/terminal.js:3622–3625` immediately lowers the refusal when hiding a web disclosure. The OSC
leave handler calls that path at `:3729–3738`; its replacement hover at `:3724–3726` reconstructs the disclosure without
restoring the raised refusal. `terminal-files.js:355–358` changes its outcome from `disclosure-refusal` at layer 46 to
ordinary `failure` at layer 12, below the disclosure at 40 and tunnel notice at 45.

The vendored `assets/vendor/xterm.js:1`, in `_handleNewLink`'s `onRenderedViewportChange` callback, clears and
re-requests the link under the last pointer position when its row range redraws. Unchanged content can therefore cause
leave and re-hover without pointer movement. The comment at `terminal.js:3730–3733` recognizes this lifecycle and
protects file links from lowering their refusal during that synchronous replacement, but the web branch still lowers it
immediately. The existing lifecycle asymmetry defeats this commit's new guarantee: `SPEC_impl.md:2050–2053` says the
explanation stays above covering surfaces until the pointer leaves.

Extend the covered-OSC browser case to repaint the link row after refusal without moving the pointer, then check actual
refusal/notice stacking again. Preserve the raised explanation when a redraw reclaims the same web hover, withdrawing it
on real departure or replacement.

Suggested bucket: `other`. Confidence: likely; the lifecycle and layers are established statically, but actual browser
geometry and typical redraw frequency were not verified. Provenance: separate same-model link reviewer in the trailing
review through `bf0935f02fc8fbfc26ab54e1b9fb8e7e6bf5c441`; the recording parent checked the cited handlers and contract.
No runtime reproduction was run.

Possible cover: none found in SPECs, Planned TODO, BUGS, queue or ledger. The ledger's
`approval-cards-can-obscure-file-disclosure.md` and `tunnel-error-notice-obscures-link-disclosure.md` require visible
refusals rather than accepting their disappearance. The rare-diagnostic filter's rarity condition is unestablished:
ordinary TUI output can redraw the row without a narrowly timed user action. Proposed drop: none.
