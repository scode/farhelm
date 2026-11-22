# Held Escape bypasses the new empty-pane highlight dismissal and closes the launcher

Reviewed commit: f6b5e5b1a39cb2b69c817a9c1f4d1153ee1f67c6

## TLDR

On Wayland, holding Escape can discard the launcher draft after dismissing a highlighted result or the hook setup
question.

## Details

**other.** Source inspection.

f6b5e5b1a39cb2b69c817a9c1f4d1153ee1f67c6 crates/gui/farhelm-gui/src/launcher/view/search.rs:430-439,
view/field_list.rs:117-138, view.rs:222-237, layers.rs:826-843,1276,1546-1575,1654-1661;
14fda80233aa69231c0b44431fc950764fbcdfd1 launcher/view.rs:227-243, launcher/view/search.rs:227-232,522-540,
launcher/view/search/start.rs:83-100, actions.rs:414,515-530,703-733, focus.rs:209-241,306-325;
vendor/gpui-pre-linux/src/linux/wayland/client.rs:2249-2289

Scope recheck: the wide pane changed the dismissal route, while application-level repeat handling stayed unchanged.
Before the split, Escape removed an ordinary non-focusing search popover. The only focus-free closing guard here is for
a submenu trigger, so the old typed-result popover could already let a held repeat reach the launcher. After the split,
dismissal of inline results returns Stay and has the same user-visible gap for typed search. The new path is the
empty-pane highlight: SPEC_impl.md:8943-8948 at 1a53d9d35889d7dde96e31320cc1681dd013737d says Escape clears that
highlight before the parent dialog handles dismissal. On repeat-enabled Wayland, the first Escape now clears the
highlight, but its held repeat closes the launcher and discards unsaved input without a fresh press. Keep LC-15 scoped
to the new empty-pane route and treat the typed-result symptom as inherited. The reviewer suggested other, confidence
confirmed by source for the repeat-enabled path; native execution not measured. ; Possible coverage: related to LC-13
only at the dismissal hook.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `launcher-command/LC-15`, with
`hook-launcher/HL-16`.
