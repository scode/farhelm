# The highlighted recent's Return action gained modified Return when focus moved into the editor

Reviewed commit: f6b5e5b1a39cb2b69c817a9c1f4d1153ee1f67c6

## TLDR

The new launcher accepts modified Return on highlighted recent setups in addition to the displayed Return shortcut.

## Details

**other.** Source inspection.

f6b5e5b1a39cb2b69c817a9c1f4d1153ee1f67c6 crates/gui/farhelm-gui/src/launcher/view/recents.rs:282-320,
launcher/view/search.rs:196-217, components/button.rs:1331-1351, components/mod.rs:408-440, layers.rs:2355-2365;
14fda80233aa69231c0b44431fc950764fbcdfd1 crates/gui/farhelm-gui/src/launcher/view/search.rs:241-256 and
view/search/start.rs:124-142; 1a53d9d35889d7dde96e31320cc1681dd013737d
crates/gui/farhelm-gui/src/launcher/view.rs:748-764, launcher/view/search.rs:245-260,
launcher/view/search/start.rs:139-146,522-541, components/text_input.rs:194-215,401-429, bootstrap.rs:84,
Cargo.lock:2826-2829,2867-2878; gpui-base 0.9.1 src/input/mod.rs:82-84 and
src/input/base/state.rs:184-211,2373-2387,2477-2487,5193-5203,12594-12599; SPEC.md:1135-1156,1382-1389,1433-1436 and
SPEC_impl.md:1237-1244 at pinned tip

Confirmed a user-visible change for the selected recent, narrowed to the exact standard bindings that reach the editable
single-line field. GPUI Kit binds Shift+Return and secondary Return (Cmd+Return on macOS, Ctrl+Return elsewhere) and
emits PressEnter with shift or secondary set. TextInput projects only held, so the launcher treats those fresh
submissions like the displayed bare Return. The old focused recent's explicit key handler and the button fallback both
rejected modifiers, and the layer's main action required an unmodified Enter with the dialog itself focused. The older
editor already accepted its own submitted Enter, so the change concerns which recent action a user can invoke, rather
than introducing modified Return to the editor generally. SPEC.md describes Enter for the highlighted row and leaves the
editor its own rules. Its Shift+Enter statement concerns cancellation of an armed template sequence, and the plain-Enter
rule in SPEC_impl.md 1237-1244 is for shared choice controls. There is no express plain-only requirement for the
editor-owned row. The reviewer suggested other for the changed and unadvertised shortcut. No claim is made for other
chords, native input timing, or actual replacement.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `launcher-matches/LM-13`.
