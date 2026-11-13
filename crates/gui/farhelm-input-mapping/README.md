# Native terminal input policy

This crate turns the key, text and pointer events the GPUI desktop app receives into the supervisor's grid input
messages (`farhelm_proto::grid::GridInput`), without depending on GPUI: the app copies its native event fields into this
crate's types. The supervisor still owns every terminal byte encoding. The crate came from the GPUI prototype of
Farhelm, where a second UI toolkit shared it; its port to the current protocol dropped the prototype's quick-switcher
matcher, which belongs to the launcher, not to terminal input.

`cursor::move_to_cell` borrows the shared grid model for pinned Alt-click behavior. It returns at most three
direction/count runs; a long move must be admitted as one ordered local input item, then expanded into individual Key
frames by the connection task. Peek is repeatable; advance only after a successful send. The existing 1,600-case move
oracle covers wrapped and ordinary screens.

NOTE: Native mapping is bounded to named keys, US ASCII inference and the explicit keypad positions described below.
Stock GPUI 0.3.12 omits physical identity and original Shift. Farhelm's native-key patch to the vendored GPUI
(`vendor/README.md`, "GPUI") supplies those optional fields; synthetic events still omit them. These tests do not
establish complete international-layout conformance or real IME coverage.

## Calling contract

Copy the actual GPUI event's modifiers, key, key_char, is_held and prefer_character_input into `KeyDown`. There is no
place to supply DOM `code` or `keyCode`. Optional `native` carries only an actual XKB canonical key name or Mac virtual
keycode, with original Shift; unknown/synthetic origin stays `None`. `map_key` accepts explicit viewer `Platform`,
owner-held `Composition`, and the current `Route`. Composition wins over app shortcuts. The caller resolves app
shortcuts against live enabled controls before terminal input; a recognized held shortcut is consumed without repeating
or reaching the terminal. The modal and focus flags come from the actual UI, not from the terminal model.

`Decision::Send` cancels the native event before automatic text insertion. Send its message once.
`Text { cancel: false }` and `Composition` leave the event available to the InputHandler. Its committed replacement
calls `committed_text`; marked replacements send nothing. A `Text { cancel: true }` result follows the existing client
policy's cancel-without-send case. Local page/select actions and app shortcuts are consumed locally. `Suppress` consumes
a held app shortcut. `Unhandled` leaves ownership with another UI control. `Unsupported` must be reported as
unsupported, rather than guessed into terminal input.

The focused InputHandler owns marked ranges and their lifecycle. Keep `Composition::marked` true through an Enter that
commits preedit, so that Enter cannot also submit or produce CR. Clear `dead_pending` after a committed insert. The
actual UI must test this lifecycle: a pure test cannot establish the order in which ibus, fcitx or AppKit invokes
callbacks. Wayland's ASCII IME commit arrives as a synthetic KeyDown rather than an InsertText; the eventual route must
still deliver exactly one character. There is no timer or content deduper, because repeated identical commits are
legitimate input.

Without native metadata the mapper sends an empty `code` rather than invent physical identity. Named DOM `key` and `kc`
are inferred from GPUI names. ASCII `kc` and removed Shift are inferred using US pairs. Native Shift replaces that
inference, and matching printable keypad digits/operators get the existing Numpad `code`/`kc`. A physical KP1 whose
logical key is End remains End; physical position does not override navigation semantics. Other physical positions do
not fabricate DOM fields or promise non-US shortcut matching. ASCII letters retain the shortcut label while `key_char`
preserves committed text. Mac Option and differing Linux text results pass to the committed Text route. Function
modifiers have no GridInput equivalent and return `Unsupported`.

Pointer helpers reuse the protocol's `MouseMode`. Apply selection/link admission before `MouseDeduper::admit` and only
record reports actually sent. Geometry uses client pixel rounding and clamping; wheel deltas use positive down and
negative up. A nonzero accumulated amount admits one report, as the existing client does. Off-mode alternate screen
wheel still goes to the supervisor as `Mouse::Wheel`; normal-screen wheel scrolls local history. Focus is reported only
while mode 1004 is enabled. UI callbacks and transport are outside this crate.

## What the tests prove

Run `cargo nextest run -p farhelm-input-mapping` (or `cargo test -p farhelm-input-mapping -- --nocapture`). The dev
dependencies use the workspace's own supervisor `InputEncoder` and terminal-emulator mode types, so a mapping that
diverges from what the supervisor sends fails here. The runtime dependencies are only `farhelm-proto` and the grid
model.

The tests replay the old web client's whole keydown decision table (332,160 platform, modifier, AltGraph and dead-key
decisions), check all 1,578 rows of the supervisor's key table against the real encoder (100,992 comparisons) and all
768 rows of its mouse table (6,144 comparisons), and replay the 1,600-case Alt-click table.

Native conformance is a separate, narrower check. The DOM tables carry fields no native event has (`code`, `keyCode`)
and deliberately inconsistent combinations, so the native tests project each key row onto what GPUI actually exposes and
keep only the rows a US layout can produce: 1,048 key-row/modifier pairs, checked for their decision and, when they
send, their bytes. The rest of the table (dead-key tuples without a physical code, empty or unidentified keys,
inconsistent key/code/keyCode combinations, IME keyCode 229, iOS-only names) has no faithful native event and is neither
a pass nor a failure. These are modeled native events, not events captured from a real desktop.

## Known limits

Without the native-key metadata, two keys can become indistinguishable. Alt+1 on the top row and Alt+1 on the keypad
reach GPUI as equal key events (the prototype confirmed this from libxkbcommon on a US layout); the supervisor sends ESC
1 for the first and nothing for the second. Without metadata the mapper takes the top-row reading, and
`standard_keypad_location_collision_cannot_preserve_both_legacy_routes` pins that loss. With the native-key patch, the
physical position tells them apart, and the metadata regression test checks both positions on Linux and macOS. Ctrl+_
has a similar, conditional ambiguity because GPUI removes Shift from single-character symbols; the mapper assumes the US
shifted pair.

The app reads the optional native metadata from GPUI's key events; this crate never reconstructs it from DOM fixture
columns, and shortcut routing by physical key belongs to the app. Real ibus, fcitx and AppKit input methods, non-US
physical shortcuts, the order of GUI callbacks, mouse capture and native scroll direction need tests in the running app.
So does Shift+Enter reaching the terminal as the right bytes: the encoder check here is not that proof.
