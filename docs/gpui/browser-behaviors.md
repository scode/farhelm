# Implicit browser behaviors in today's UI (reference commit 24bc0580)

Inventory for the GPUI port. Each entry names a behavior the Dioxus UI gets from the browser or WKWebView without
writing it down, the evidence for it, and a mark:

- **keep**: SPEC names it, or a user plainly depends on it. "GPUI home" says where it naturally belongs. Keep means the
  area that owns the surface ports the behavior when it ports that surface; it is not a list of things to build in M1,
  where a foundation is built only if the end-to-end slice consumes it.
- **drop**: exists only because a browser provides it; nothing in SPEC or the product depends on it.
- **ask**: was unclear; every such entry now records the maintainer's decision (Decisions, at the end).
- **record only**: noted, not built: IME composition and screen-reader extras beyond roles and labels (the maintainer
  decided both are low priority), and right-to-left text and dead keys beyond what GPUI gives.

Some entries carry both a keep line and a drop line: the browser widget goes, but an obligation it met today stays.

Conventions. Code and test line numbers are at 24bc0580. `ui/` means `crates/farhelm-ui/`; `e2e/` specs are under
`e2e/tests/`. "SPEC" is `SPEC.md` and "SPEC_impl" is `SPEC_impl.md`, cited by section heading and a short quote from the
current text rather than by line, since SPEC was rewritten for the desktop-only GUI after 24bc0580. Where a quote exists
only in the 24bc0580 text, the entry says so. "dioxus-desktop" means the crates.io source of dioxus-desktop 0.7.10
(Apache/MIT), read to confirm what the Mac app's webview shell does.

Scope: the GPUI app replaces the desktop app, and the browser UI is dropped (SPEC.md, Topology). Behaviors that existed
only for browser clients (touch, phone widths, browser tabs) are marked against the desktop app. The entries first
marked ask were decided by the maintainer on 2026-11-13; see Decisions at the end.

Measured vs. inferred: every code and test citation was read at 24bc0580. Statements about what WKWebView or a browser
does by default (page zoom in the Mac app, primary-selection behavior on Linux) are inferred, not tested, and say so.

74 entries: 51 keep, 24 drop, 4 record only. D3, D9, E5, F5 and L9 carry one keep line and one drop line and count under
both.

## A. Text selection and clipboard

**A1. Selecting and copying non-input text (labels, help, dialog text, header, session titles)** — **keep**

- SPEC, "Session list": "Shift marking does not extend the page's text selection, and ordinary dragging still selects
  text." SPEC, "Opening and closing" (dialogs): "A primary click on empty space keeps focus on the control in use ...;
  text can still be selected."
- Code: `ui/assets/app.css` leaves text selectable by default. It turns selection off only during the app's own drags
  (app.css:862-865 sidebar resize, 1042 window drag strip, 2814 pin drag) and on the terminal grid, which has its own
  selection (app.css:8636-8637). Copy of a page selection in the Mac app comes from the native Edit menu
  (`ui/src/desktop/menu.rs:58-67`).
- Tests: `settings.spec.ts:806-816` ("dragging over help text must still select it"); `window-chrome.spec.ts:183-193`
  (double-click on the version selects a word); `bulk-session-actions.spec.ts:~395-470` (a selection in a session title
  survives Shift-click marking); `launcher-enter.spec.ts:177` (a click on an empty gap selects nothing).
- GPUI home: component layer (a selectable-text element: drag, double/triple click, Cmd+C, selection suppressed while an
  app drag runs). GPUI text is not selectable out of the box, so this is a sizable piece.

**A2. Terminal selection and copy** — **keep**

- SPEC, "Terminal experience": "Selecting text copies it to the system clipboard: a plain drag when the pane has no
  mouse reporting active, or Shift-drag (Option-drag on macOS) to force a local selection".
- Already custom (`ui/assets/grid-terminal.js` header, "Selection and copy"; `copy-on-select.js`). The browser part is
  the native `copy` event that Cmd+C (Edit menu) fires on the hidden input (grid-terminal.js:1563) and Cmd+A as
  select-all (`ui/assets/grid-input.js:242-243`).
- Tests: `terminal-clipboard.spec.ts` (458, 525, 568, 618, 687).
- GPUI home: terminal; Cmd+C / select-all through actions/keymap.

**A3. Clipboard writes for copy buttons (directory, command line, Copy plan, Copy ID)** — **keep**

- SPEC, "Terminal experience": "Clipboard writes use the system clipboard and fail silently." and "Clipboard operations
  are explicitly best-effort and silent on failure".
- GPUI home: actions (platform clipboard write; keep the silent-failure contract).

**A4. Clipboard permission prompts and secure-context rules** — **drop**

- At 24bc0580, SPEC's "Terminal experience" named "permission policy, secure-context requirements" only as reasons
  clipboard failures are silent; the current text reads "the operating system's clipboard behavior is outside this
  system's control". GPUI writes the platform clipboard directly. The silent-failure contract (A3) stays.

## B. Context menus

**B1. The browser's own context menu (Copy, Paste, Look Up, Inspect) on text, fields, links and the terminal** —
**drop**

- The Mac app already has none: `ui/src/desktop.rs:223` `.with_disable_context_menu(!cfg!(debug_assertions))`, which
  dioxus-desktop implements as a document-wide `contextmenu` preventDefault (dioxus-desktop `src/webview.rs:443-453`).
  Only browser clients get it. The terminal moves its hidden input under the pointer so a browser's menu offers Paste
  (grid-terminal.js:1589-1599, 1844-1847).
- SPEC: nothing. Tests: none.

**B2. App-defined context menu: right-click, the ContextMenu key or Shift+F10 on a host's rail mark opens its menu** —
**keep**

- `ui/src/hosts.rs:811-818`. User docs: `website/src/content/docs/docs/using/manage-hosts.mdx:38` "right-click the
  host's mark on the navigation rail".
- Tests: `filters.spec.ts:678`, `host-rail-warning.spec.ts:102, 301`, `provisioning.spec.ts:440, 475`,
  `nonblocking-launch.spec.ts:712`.
- GPUI home: layer manager (menu) + actions/keymap (ContextMenu, Shift+F10).

## C. Keyboard focus

**C1. Tab and Shift+Tab follow document order** — **keep**

- SPEC, "Opening and closing": "Every modal dialog keeps Tab and Shift+Tab within its visible, enabled controls and
  keyboard scroll regions." SPEC, "Terminal experience": "Keyboard order follows the top row, terminal, then bottom
  bar." SPEC, "Session list" (Settings): "Section buttons follow the platform's normal Tab rules."
- Tests: `modal-trap.spec.ts` (11 Tab presses), `sidebar.spec.ts` (15), `trash-dialog.spec.ts` (8),
  `restart-with.spec.ts`, `launcher-template-shortcuts.spec.ts`, `templates.spec.ts` and about 30 more files.
- GPUI home: focus coordinator (explicit order per region; containment for modals).

**C2. macOS Tab skips buttons unless Keyboard navigation is on (Option-Tab reaches them)** — **keep** (decided
2026-11-13)

- SPEC, "Reaching everything by keyboard": "On macOS, keyboard-only users reach buttons with Keyboard navigation or
  Option-Tab (the default Tab order includes only text fields and pop-ups), so button-only regions need no extra stop."
- `e2e/tests/chrome.spec.ts:177-183` notes WebKit's plain Tab skips buttons, so the keyboard half of the ring test runs
  only in Chromium.
- Decided: on macOS, Tab follows the system Keyboard navigation setting exactly as today, and Option-Tab reaches
  buttons, per SPEC; on Linux, Tab reaches every focusable control.
- GPUI home: focus coordinator.

**C3. Focus rings only for keyboard-arrived focus (`:focus-visible`)** — **keep**

- SPEC, "Things changing underneath": a race never runs a command "that the user did not activate on a control that was
  visibly focused when they activated it". SPEC_impl, "GUI: Dioxus": "retains the normal `:focus-visible` indicator, so
  pointer-return focus and keyboard focus remain visually distinct"; "Modal dialog containers have no focus ring"; hover
  help opens when "keyboard focus (`:focus-visible`) reaches one".
- Code: 67 `:focus-visible` rules in `ui/assets/app.css`.
- Tests: `chrome.spec.ts:145-200`, `buttons.spec.ts:85, 192-202`, `menu-highlight.spec.ts:235-276`,
  `terminal-tabs.spec.ts`, `tooltip.spec.ts`.
- GPUI home: focus coordinator (track whether focus arrived by key or pointer) + theme (ring token).

**C4. Focus returns to where it was when something closes, whichever engine decided click focus** — **keep**

- Today whether a click focuses the clicked button differs by engine (WebKit: no; Chromium: yes). Only the SPEC rule
  below is ported; the engine difference is not.
- SPEC, "Opening and closing": "Closing something the user opened returns focus to where it was before it opened, on
  every platform, including those where clicking the button that opened it did not move focus to the button."
- GPUI home: focus coordinator (record the return target at open time, independent of click focus).

**C5. Initial focus when a dialog opens (today the `autofocus` attribute)** — **keep**

- Only the initial-focus rules are ported; the `autofocus` attribute mechanism is not.
- SPEC, "Opening and closing": "A confirmation the user just opened starts on the action they asked for"; the section
  defines starting focus per dialog.
- Code: `ui/src/auth.rs:506`, `ui/src/hosts.rs:292, 5527`, `ui/src/hosts/settings_dialog.rs:891, 922`,
  `ui/src/list/row.rs:969-974`, `ui/src/lib.rs:252` (browsers consume the first autofocus opportunity),
  `ui/src/list/bell.rs:306` (explicit focus instead).
- GPUI home: focus coordinator.

**C6. Focusing a control scrolls it into view; explicit `scrollIntoView`** — **keep**

- SPEC, "Creation" (YOLO confirmation): "shown with the control or surface that started the launch and scrolled into
  view". SPEC, "Session list" (Settings): "Trust hints instead open agent trust at the relevant switch and scroll its
  block into view." SPEC, "Desktop window chrome" (quick switcher): "Every match is reachable by scrolling".
- Code: `ui/src/yolo_confirm.rs:191-201`, `ui/src/settings.rs:347`, `ui/src/list/quick_switcher.rs:333`,
  `ui/src/list/host_picker.rs:107`, `ui/src/list/create_form.rs:2243-2259`, `ui/src/launch_controls.rs:668`,
  `ui/src/list/view.rs:1649`, `ui/src/list/trash.rs:662`, `ui/src/list/templates.rs:2532`,
  `ui/src/tensorlake/recipe_editor.rs:843`; `ui/assets/app.css:520-528` notes focus() scrolls even an overflow-hidden
  viewport.
- Tests: `github-checkout-composer.spec.ts:161` ("keeps the active suggestion visible"), `keyboard-scroll.spec.ts`.
- GPUI home: focus coordinator + scroll container (an ensure-visible call on focus and on highlighted-item change).

**C7. Window activation, hiding and focus restore on return** — **keep**

- SPEC, "Who may move focus": "switching to another app and back do not count" (as moving on). SPEC, "Creation": "Escape
  cancels and is consumed; a click, switching windows or ..." ends a quick-launch sequence. SPEC_impl, "GUI: Dioxus":
  "Every looping animation also stands still while the window is inactive". A browser also restores the focused element
  when the window regains focus.
- GPUI home: focus coordinator + window (activation events; restore the focused element; pause animations).

## D. Activation and forms

**D1. Buttons activate on Enter and Space** — **keep**

- SPEC_impl, "GUI: Dioxus": "Nothing activates without Enter or Space on the visibly focused item." SPEC, "Opening and
  closing": "Held or composing Enter cannot confirm."
- Code: 343 `r#type: "button"` in `ui/src`.
- Tests: `buttons.spec.ts`, `menu-highlight.spec.ts`, `session-confirmations.spec.ts`.
- GPUI home: component layer (button) + actions/keymap (repeat and composition filtering).

**D2. Enter in a single-line field submits its form** — **keep**

- SPEC, "Creation" (templates): "Enter in the name field saves without launching". SPEC, "Opening and closing": "If a
  dialog container itself has focus, Enter presses its enabled main action".
- Code: forms at `ui/src/auth.rs:445`, `ui/src/hosts.rs:5501, 6101`, `ui/src/rename.rs:110-150` (a textarea that
  emulates single-line submit), `ui/src/list/create_form.rs:7765`, `ui/src/modal_dialog.rs:1038-1040`.
- Tests: `launcher-enter.spec.ts`, `templates.spec.ts`, `terminal-replay-rename.spec.ts`.
- GPUI home: component layer (input emits a submit action) + actions/keymap.

**D3. Required fields** — **keep** the refusal, **drop** the browser's bubble

- Keep: Add host refuses an empty SSH destination with a visible reason of its own. GPUI home: component layer (a
  required-field check the dialog shows beside Add).
- Drop: the browser's native validation bubble.
- Evidence: `ui/src/hosts.rs:6259` marks Add host's SSH destination `required`; its submit button is disabled only while
  busy, so today the browser's own bubble is what stops an empty submit. The feedback box (`ui/src/feedback.rs:327`) is
  also `required`, but Send is disabled when invalid (feedback.rs:195-205). The launcher is `novalidate`
  (`ui/src/list/create_form.rs:7732-7734`, "Browser validity bubbles cannot explain a refusal beside Launch"). SPEC:
  nothing.

**D4. Checkboxes, radios, and clicking a label to toggle its control** — **keep**

- 25 checkboxes, 5 radios; labels wrap their inputs (`ui/src/appearance.rs:30-50`, `ui/src/list/view.rs:6043-6056`).
  `accent-color` tints native controls (app.css:545).
- GPUI home: component layer.

**D5. Radio groups: one Tab stop, arrow keys move the choice** — **keep**

- Code: native radios in `ui/src/list/view.rs:6043` (status filter), `ui/src/appearance.rs:39`,
  `ui/src/settings.rs:1112`, `ui/src/list/create_form.rs:9008, 9022`.
- Tests: `status-filter.spec.ts:51`, `appearance.spec.ts:315-324`, `settings.spec.ts:1965`.
- GPUI home: component layer (segmented control / radio group).

**D6. Native `<select>` (keyboard open, type-ahead, arrows, platform popup)** — **keep**

- 8 selects: `ui/src/environment_ui.rs:825, 876`, `ui/src/launch_controls.rs:392`, `ui/src/list/create_form.rs:9038`,
  `ui/src/list/templates.rs:1193, 1411`, `ui/src/tensorlake/add_host.rs:327`, `ui/src/tensorlake/recipe_editor.rs:987`.
- SPEC, "Reaching everything by keyboard": "the default Tab order includes only text fields and pop-ups".
- Tests: `selectOption` in `templates.spec.ts` (8), `tensorlake-hosts.spec.ts` (8), `podman-environments.spec.ts` (4),
  `launcher-enter.spec.ts`, `credentials-settings.spec.ts` and others.
- GPUI home: component layer (dropdown) + layer manager (popup).

**D7. File input opens the system file picker** — **keep**

- SPEC, "Session list" (Credentials): "Codex accepts a pasted or selected `auth.json`". Code:
  `ui/src/credentials_settings.rs:369`.
- GPUI home: component layer, using the platform open panel.

**D8. Password fields mask their text** — **keep**

- SPEC, "Session list" (Credentials): "a write-only secret field". Code: `ui/src/auth.rs:501`,
  `ui/src/credentials_settings.rs:361`, `ui/src/ntfy_settings.rs:124`, `ui/src/tensorlake_settings.rs:204`.
- GPUI home: component layer (masked input mode; no copy-out).

**D9. Number fields** — **keep** the whole-number rules, **drop** the browser's number widget

- Keep: a recipe's CPUs, memory and disk accept only whole numbers of at least 1, and its tunnel ports only 1 to 65535.
  GPUI home: component layer (a numeric text field with bounds).
- Drop: step arrows, Up/Down increments, wheel stepping and the engine's own digit filtering.
- Evidence: SPEC, "Topology" (recipes): "(CPUs, memory in GB and disk in GB, whole numbers)". Code:
  `ui/src/tensorlake/recipe_editor.rs:620-632` (`min: "1"`, `step: "1"`), 718-726 (tunnel ports, `min: "1"`,
  `max: "65535"`). SPEC does not name the port range; the code does.

**D10. `type=search` clear button and Escape-clears, `type=url`, `inputmode` keyboards** — **drop**

- `ui/src/list/create_form.rs:7899`, `ui/src/ntfy_settings.rs:106`, `ui/src/tensorlake/recipe_editor.rs:1022`. SPEC:
  nothing.

**D11. Browser autofill and password managers** — **drop**

- Only `ui/src/feedback.rs:340` (`autocomplete: "email"`) and `ui/src/credentials_settings.rs:362` (`new-password`) opt
  in; every other field sets `autocomplete: "off"`. SPEC: nothing.

**D12. Native `<progress>` bar** — **keep** (visual only)

- `ui/src/hosts/attention_list.rs:383-388` (host update progress in the attention list).
- GPUI home: component layer.

**D13. `<details>`, `<summary>`, `<dialog>`, `<datalist>`** — **drop** (not used anywhere in `ui/src`)

## E. Text editing in fields

**E1. Caret movement and selection (arrows, word jumps, Home/End, Cmd+arrows, Shift-extend, double-click word, Cmd+A,
drag-select)** — **keep**

- SPEC, "Desktop window chrome": "Text fields outside the session launcher keep their normal Cut or text-direction key
  ... In the launcher's own fields, selected text keeps that normal action". SPEC, "Lifecycle operations" (rename): "its
  draft, selection, composition, and source session identity stay intact".
- Tests: Home/End in `feedback.spec.ts:201-203`, `clone.spec.ts:442`, `launcher-enter.spec.ts:1098`,
  `settings.spec.ts:1325`, `host-picker.spec.ts:65, 75`.
- GPUI home: component layer (text input). GPUI ships no finished text field.

**E2. Cut, Copy, Paste, Undo, Redo, Select All in fields** — **keep**

- In the Mac app these come from the native Edit menu `ui/src/desktop/menu.rs:58-67`. SPEC, "Desktop window chrome":
  "keep their normal Cut".
- GPUI home: actions/keymap + app menu + text input (undo stack per field).

**E3. Multi-line fields: Enter inserts a newline, Tab leaves the field, read-only fields stay selectable, placeholders**
— **keep**

- Textareas: `ui/src/checkout_post_clone.rs:63`, `ui/src/credentials_settings.rs:356`, `ui/src/feedback.rs:318`,
  `ui/src/launch_controls.rs:599`, `ui/src/list/templates.rs:1284`, `ui/src/rename.rs:130`,
  `ui/src/tensorlake/recipe_editor.rs:700`. `readonly` on locked fields (e.g. `checkout_root.rs`,
  `credentials_settings.rs`).
- GPUI home: component layer.

**E4. Drag grip that resizes a text box (`resize: vertical`)** — **drop** (decided 2026-11-13)

- app.css:3597 (feedback message), 3824 (templates), 4038, 4117 (recipe editor), 4170 (credential), 7065 (initial
  prompt: "It can grow vertically inside the scrolling body"). Rename uses `resize: none` (app.css:4498). SPEC: nothing.
  Tests: none found.
- Decided: no resize grip. Multi-line boxes grow with their text up to a cap, then scroll.
- GPUI home: component layer (the multi-line text box).

**E5. Spellcheck, autocorrect, autocapitalize, smart substitutions** — **keep** the opt-out, **drop** the browser
features

- Keep: no field corrects, capitalizes or substitutes what the user types; the text input must not opt into macOS text
  substitution, spellcheck or autocorrect through its input handler. GPUI home: component layer (text input).
- Drop: the browser's spellcheck and correction machinery and the attributes that turn it off.
- Evidence: the app opts out everywhere: `ui/src/list/create_form.rs:8338-8350` (WKWebView autocorrect silently changed
  "claude" to "Claude"), `ui/src/feedback.rs:320-325` ("the app's no-correction policy includes this prose box"),
  `ui/src/hosts.rs:5473, 6253-6263`, `ui/assets/grid-terminal.js:372-374`, and every other field.

**E6. IME composition** — **record only**

- SPEC, "Terminal experience": "Shift+Enter (the exact chord — no other modifier held, not mid-IME-composition)". SPEC,
  "Lifecycle operations": rename keeps "its draft, selection, composition". SPEC, "Opening and closing": "Repeated or
  composing Enter cannot answer the new question." SPEC, "Desktop window chrome": "ignore repeat and composition".
- Code: `ui/assets/composition-guard.js`; `ui/assets/grid-terminal.js:1533-1540` (committed text is sent; showing the
  composition at the cursor is "Not built", file header); `ui/src/rename.rs:139-150`.
- Tests: `ime-composition.spec.ts`.

**E7. Keyboard layout translation, physical-key matching, key repeat** — **keep**

- SPEC, "Creation": "Matching uses the physical digit key". SPEC, "Terminal experience": "The keys are the ones at the
  US-layout positions of = and −". SPEC, "Desktop window chrome": "Matching uses the physical Slash key", and shortcuts
  "ignore repeat and composition".
- Code: `ui/assets/grid-input.js:226-240` (Option as Meta), `ui/assets/terminal.js:2635-2660`.
- Tests: `terminal-keys.spec.ts`, `keyboard-shortcuts.spec.ts`, `launcher-template-shortcuts.spec.ts`.
- GPUI home: actions/keymap + terminal.

**E8. Dead keys (accent sequences such as ´ then e)** — **record only** (what GPUI gives)

- A browser routes a dead-key sequence through the same composition events as an IME, so it rides E6's path. The one
  explicit case in code is Option as Meta meeting a dead key in the terminal (`ui/assets/grid-input.js:236-240`). SPEC
  does not name dead keys.

## F. Pointer

**F1. Hover styles** — **keep**

- 64 `:hover` rules in app.css. SPEC_impl, "GUI: Dioxus": "the 100 ms hover tints". SPEC, "Session list": "Hovering an
  eligible pinned row reveals a grip". SPEC, "Terminal experience" (split menus): "mouse or pen movement focuses the
  hovered choice without scrolling".
- GPUI home: component layer.

**F2. Cursor shapes** — **keep**

- 53 `cursor:` rules: col-resize (app.css:846, 863), text over the terminal (8634), grab/grabbing for pins (2810), hand
  on terminal links (grid-terminal.js header, "Links").
- GPUI home: component layer.

**F3. Pointer capture during drags; a lost capture cancels** — **keep**

- SPEC, "Session list": "Escape, interrupted pointer capture or releasing outside the pinned group cancels."
- Code: `ui/assets/sidebar-width.js:130, 154`; `ui/assets/pin-order.js:145-176`.
- Tests: `sidebar-resize.spec.ts`, `pins.spec.ts`.
- GPUI home: component layer (drag helper).

**F4. OS click count (double and triple click)** — **keep**

- SPEC, "Session list": "Double-click the edge to reset it". SPEC, "Desktop window chrome": "double-clicking there zooms
  it". Terminal word and line selection.
- Code: `ui/assets/click-detail.js`.
- Tests: `sidebar-resize.spec.ts:138`, `window-chrome.spec.ts`.
- GPUI home: component layer + window chrome.

**F5. Hover help** — **keep** Farhelm's own hover help, **drop** native `title` tooltips

- Keep: SPEC, "Session list": "hover text appears about 300 ms after the pointer rests on a control or icon that has it,
  or keyboard focus reaches one" and "Every clickable control and every icon that carries meaning has hover text". Code:
  `ui/assets/tooltip.js`, `data-tooltip` attributes. Tests: `tooltip.spec.ts`. GPUI home: layer manager (tooltip
  layer) + focus coordinator (opens on keyboard focus).
- Drop: SPEC_impl, "GUI: Dioxus": "Hover help is Farhelm's own tooltip" and "So no element keeps a `title` as hover
  help". `tooltip-coverage.spec.ts:25, 106-127` asserts no element has a native `title`.

**F6. Ellipsis on overflow, full text in the tooltip** — **keep**

- SPEC, "Session list": "long names truncate and remain available in full in the tooltip". SPEC, "Terminal experience":
  directory and command line "ellipsize before the session name or host when the bar runs out of room".
- Code: 44 `text-overflow` rules in app.css.
- GPUI home: component layer (truncating label wired to the tooltip).

## G. Links

**G1. Links in the interface open in the system browser** — **keep**

- `<a target="_blank">`: `ui/src/welcome.rs:62-63`, `ui/src/announcements.rs:96-97`, `ui/src/feedback.rs:279-280`,
  `ui/src/list/bell.rs:458-462`, `ui/src/checkout_spare.rs:73`, `ui/src/list/create_form.rs:9255`,
  `ui/src/provisioning.rs:1517`. Help menu items say "Opens ... in your browser" (`ui/src/app_bar.rs:169-178`).
- In the Mac app the webview shell routes them: dioxus-desktop `src/webview.rs:381-388` (external http(s) navigation
  goes elsewhere) and `src/app.rs:275-285` (`webbrowser::open`).
- GPUI home: actions (open a URL through the platform).

**G2. Terminal links open on click** — **keep** (already custom)

- SPEC, "Terminal experience": "Links in terminal output open their http(s) target on click".
  `ui/assets/terminal-links.js:53-74` (navigates the page under `dioxus:` so the shell opens the system browser;
  `window.open` elsewhere). Tests: `terminal-links.spec.ts`.
- GPUI home: terminal.

**G3. Middle-click, Cmd-click or Shift-click on a link to open a tab or window; hover URL in a status bar; "Copy link
address"; dragging a link** — **drop**

- Browser-only. The terminal's own Cmd-click stays with G2: SPEC, "Terminal experience": "this also applies to Cmd-click
  tunnel opens" and "Cmd-click on macOS (Ctrl-click elsewhere) also opens the completed save".

## H. Drag and drop

**H1. Dropping or pasting files, images and text into a terminal** — **keep**

- SPEC, "Attachments": "Pasting or dropping content into any of a session's terminals ... Plain text passes through as
  ordinary terminal input. Dropped directories are rejected with a visible error".
- Code: `ui/assets/terminal.js:2386-2402`, 2595-2625 (a page-wide guard so a stray file drop never navigates away, and a
  drop on a pane with no live terminal says why); `ui/src/desktop.rs:176-215` (why the native drop handler is off) and
  253 (`.with_disable_drag_drop_handler(true)`).
- Tests: `terminal-attachments.spec.ts`.
- GPUI home: terminal + a window-level drop target.

**H2. Dragging text into a text field inserts it** — **drop** (decided 2026-11-13)

- `ui/assets/terminal.js:2554-2570` (`browserHandlesDrop`) deliberately lets text drops through to fields while refusing
  files there. SPEC: nothing. Tests: none found.
- Decided: paste is enough; text fields take no drops. Drops on a terminal are unchanged (H1).

**H3. Dragging selected text or images out of the app** — **drop** (SPEC: nothing; no code relies on it)

## I. Scrolling

**I1. Wheel and trackpad scrolling, with inertia, in every scroll region; horizontal scrolling of the tab strip and of a
too-narrow window** — **keep**

- SPEC, "Terminal experience": the tab strip "scrolls horizontally when needed". SPEC, "Session list": "a narrower
  window scrolls horizontally". SPEC, "Desktop window chrome": "so horizontal scrolling cannot move application controls
  underneath native window buttons".
- The terminal wheel is already custom (`ui/assets/grid-terminal.js:1820-1843`).
- GPUI home: component layer (scroll container).

**I2. Keyboard scrolling of a focused scroll region (arrows, PageUp/PageDown, Space, Home/End)** — **keep**

- SPEC, "Reaching everything by keyboard": "Every scrollable region in the GUI can be reached and scrolled with the
  keyboard alone ... A region whose content holds nothing focusable gets its own tab stop with an accessible name."
- Code: `ScrollRegion` in `ui/src/modal_dialog.rs:518`.
- Tests: `keyboard-scroll.spec.ts`, `helpers/keyboard-scroll.ts`, `trash-dialog.spec.ts`.
- GPUI home: component layer (scroll region as a tab stop) + focus coordinator.

**I3. Scrollbars: visible and draggable in dialogs; hidden in sidebar, rail and tab strip; thin in a few places** —
**keep**

- Hidden: app.css:617, 645, 659, 830, 1641-1643, 2570-2582. Thin: app.css:7719, 9520. The terminal draws its own.
- GPUI home: component layer + theme.

**I4. Scroll chaining and overscroll rubber-band** — **drop**

- `ui/assets/app.css:520-546`: the document refuses chaining and the root rubber-band on purpose
  (`overscroll-behavior: none`). Nothing asks for chaining between inner scrollers.

**I5. Smooth scrolling** — **drop** (no `scroll-behavior` anywhere; `scrollIntoView` calls are instant)

**I6. Document scrolling (Space scrolls the page)** — **drop** (the document never scrolls, app.css:520)

## J. Appearance, motion and accessibility

**J1. Follow the system light/dark appearance, live** — **keep**

- SPEC, "Session list" (Settings, Appearance): "System follows the device's current appearance, including changes while
  Farhelm stays open".
- Code: `ui/assets/appearance.js:35`; `ui/src/desktop/appearance_smoke.rs:94, 193`.
- Tests: `appearance.spec.ts`.
- GPUI home: theme (observe the window's appearance).

**J2. Reduced motion** — **keep**

- SPEC, "Topology" (host progress): "Reduced-motion settings replace the animated indicator with a static one". SPEC,
  "Installation and updates": "either spinner stops moving under reduced motion". SPEC, "Creation": "Reduced motion
  replaces slide and fade" and "disabled for reduced motion". SPEC, "Managed checkouts": "static but still upside down
  under Reduce Motion" and "Reduced motion skips both parts". SPEC_impl, "GUI: Dioxus": "every looping indicator is
  static".
- Code: 11 `@media (prefers-reduced-motion: reduce)` blocks in app.css (915, 1281, 1760, ...);
  `ui/src/list/trash.rs:1535`.
- Tests: `permission-switch.spec.ts:291`, `update-readout-css.spec.ts:97`, `trash-dialog.spec.ts:603`,
  `provisioning.spec.ts:2908, 2957`, `github-checkouts.spec.ts:1626`, `quick-launch.spec.ts:654`.
- GPUI home: theme (motion setting read from the OS accessibility preference).

**J3. Forced colors and increased contrast** — **drop**

- No `forced-colors` or `prefers-contrast` rules; SPEC: nothing.

**J4. Accessible names, roles, states and descriptions** — **keep**

- SPEC, "Session list": "their hover text and accessible names explain the actions" and "Each avatar's tooltip and
  accessible name". SPEC, "Terminal experience": "their tooltips and accessible descriptions explain the specific
  reason". SPEC, "Reaching everything by keyboard": "its own tab stop with an accessible name". SPEC, "Local authority
  and trust between hosts": "direction-changing characters are visible, including in accessible names".
- Code: 239 `aria_label` in `ui/src`, plus roles and `aria-*` states throughout.
- Tests: about 2,588 `getByRole` calls across `e2e/tests` locate controls by role and name, so these names also carry
  the test suite.
- GPUI home: component layer (accessibility properties per element). Not checked here: how much of this GPUI's
  accessibility support can express.

**J5. Screen-reader announcements (live regions)** — **record only** (watch list)

- SPEC, "Installation and updates" and "Creation": "announced to screen readers". About 140
  `aria-live`/`role: "status"`/`role: "alert"` sites.
- Screen readers are low priority by the maintainer's decision: roles and labels go into the component constructors, but
  live announcements wait on the watch list until there are enough users to justify them.

**J6. Bidirectional text in labels** — **record only** (decided 2026-11-13)

- The browser runs the Unicode bidi algorithm on every label. Many fields force `dir: "ltr"` (e.g. 18 in
  `ui/src/hosts.rs`, 9 in `ui/src/approvals.rs`); the terminal overrides bidi to left-to-right
  (`ui/assets/grid-terminal.js` header). SPEC, "Local authority and trust between hosts", makes direction-changing
  characters visible in host-supplied text.
- Decided: right-to-left text gets only what GPUI gives; nothing extra is built. The host-text rule (direction-changing
  characters made visible) is unaffected.

**J7. Accessible names taken from visible text** — **keep**

- A browser names a control from its visible text when no explicit label is given. Most plain-word buttons rely on that:
  Cancel and Close in `ui/src/feedback.rs:405-417`, Cancel in `ui/src/list/row.rs:2509, 2582`, and many more carry only
  their text and a `data-tooltip`.
- Tests find them by that name: `getByRole("button", { name: "Cancel", exact: true })` in
  `bulk-session-actions.spec.ts:47` and `buttons.spec.ts:96, 247`; `name: "Close"` in `claude-trust.spec.ts:95`.
- GPUI home: component layer (derive the accessible name from the visible label when no explicit one is given).

## K. Fonts, glyphs and text layout

**K1. Bundled fonts with system fallback** — **keep**

- `ui/assets/app.css:47-91` (`@font-face` for Inter and JetBrains Mono Nerd Font), 370-371
  (`--font-ui: "Inter", system-ui, sans-serif; --font-mono: "JetBrains Mono", monospace`), 570-585 (form controls
  inherit the face); `ui/assets/terminal.js:369, 409-434` (the terminal waits for both JetBrains weights).
- Tests: `interface-fonts.spec.ts`, `terminal-font.spec.ts`.
- GPUI home: theme (font registration and fallback chain) + terminal.

**K2. Emoji and other glyphs the bundled fonts lack (colour emoji, CJK, symbols)** — **keep**

- SPEC, "Terminal experience": "Full fidelity ... If it works over plain SSH it must work here."
  `ui/assets/grid-terminal.js` header (a character from a fallback font is boxed to its cell width).
- Tests: `terminal-unicode.spec.ts` (VS16 emoji widths across live output and replay).
- GPUI home: terminal + text system (font fallback).

**K3. Display scale changes (moving the window between displays)** — **keep**

- `ui/assets/grid-terminal.js:443, 1946-1947` re-measure the cell on a `resolution` change; the fit contract
  (grid-terminal.js header, "Size") depends on it.
- GPUI home: terminal (react to the window's scale factor).

**K4. Wrapping and no-wrap rules** — **keep**

- SPEC, "Agent-spawned sessions" (approval cards): "Paths, repository names, SSH destinations and session IDs stay on
  one line and scroll sideways within their own values; URLs and permission flags wrap in full."
- Code: about 128 `white-space: nowrap`, `overflow-x: auto`, `overflow-wrap` and `word-break` rules in app.css, for
  example `.approval-fact-scroll` (app.css:8987-8993) and the rename field (`white-space: pre; overflow-x: auto`,
  app.css:4485-4500).
- Tests: `approval-layout.spec.ts`.
- GPUI home: component layer (a single-line value that scrolls sideways; wrapping text with break-anywhere).

## L. Window and document

**L1. Page zoom (Cmd/Ctrl with +, −, 0; pinch)** — **drop** (decided 2026-11-13)

- At 24bc0580, SPEC's "Terminal experience" said "Cmd/Ctrl with = or − and no Shift stay the browser's own page zoom";
  the current text reads "Cmd/Ctrl with = or − and no Shift are not app shortcuts and reach a focused terminal like any
  other key: there is no interface zoom". Code: `ui/assets/terminal.js:2643-2645`;
  `website/src/content/docs/docs/using/keyboard-shortcuts.mdx:84`.
- Unverified: whether page zoom works in the Mac app today. Its menu has no View menu (`ui/src/desktop/menu.rs:42-73`),
  and WKWebView by default does not zoom on Cmd+= or pinch (not tested here).
- Decided: no whole-interface zoom. Terminal text size (Cmd/Ctrl+Shift with + or −) is separate, SPEC-named and kept;
  SPEC.md no longer reserves Cmd/Ctrl with = or − for zoom.

**L2. Find in page (Cmd/Ctrl+F)** — **drop**

- No SPEC line, no code; the Mac app's Edit menu has no Find (`ui/src/desktop/menu.rs:58-67`). Search in terminal
  scrollback would be a new feature, not a port.

**L3. Print** — **drop** (no code, no SPEC)

**L4. Back and forward (keys, swipe, mouse buttons 4 and 5)** — **drop**

- No routing (no `pushState`/`popstate` in `ui/`); the desktop shell refuses navigation after the first load
  (dioxus-desktop `src/webview.rs:376-378`).

**L5. Reload (Cmd+R, F5)** — **drop** (decided 2026-11-13)

- At 24bc0580, SPEC used reload as the way other clients pick up shared changes (Session list, Terminal experience,
  Sending feedback, One GUI at a time) and as browser sign-in recovery (Signing in again: "a literal page reload"). The
  current SPEC words these around reconnects, for example "Session list": "on the same session after a reconnect as
  before it". The Mac app offers no reload in release builds (no menu item; context menu disabled,
  `ui/src/desktop.rs:216-223`).
- Decided: no reload command. With one UI per helm and the helm pushing every change to it, there are no other clients
  to catch up, and changes arrive live.

**L6. Window title from `<title>`** — **drop**

- The Mac app uses a static native title "farhelm", hidden on macOS (`ui/src/desktop.rs:302-309`); the web build's
  `<title>` is dx's default (`ui/Dioxus.toml:8-10`). The GPUI window keeps a static title for the Window menu.

**L7. Native app menu: Window and Edit menus, no Close Window, fullscreen** — **keep**

- SPEC, "Desktop window chrome": "The Mac app's Window menu has no Close Window item, leaving Cmd+W to session
  deletion". `ui/src/desktop/menu.rs:1-73`. This is the webview shell's, not the browser's, but the port has to rebuild
  it, and E2 depends on it.
- GPUI home: actions/keymap + app menu.

**L8. Per-device preferences kept in the webview's local storage** — **keep**

- SPEC, "Session list": "appearance, sidebar width, host rail expansion, terminal text size and sound switches, by
  contrast, are deliberately per device" and (Appearance) "The desktop app remembers the choice locally, independently
  of the helm". SPEC, "Losing the connection to the helm": "What survives a reset: this device's local preferences
  (sidebar width, theme, window size and the like)".
- Code: `localStorage` in `ui/assets/sidebar-width.js:53, 59, 89, 109` (width, rail), `ui/assets/terminal.js:384, 2710`
  (terminal text size), `ui/assets/appearance.js:31` (appearance), `ui/assets/sounds.js:13, 108` (sound switches).
- Tests: `sidebar-resize.spec.ts`, `appearance.spec.ts`, `terminal-font.spec.ts`, `sounds.spec.ts`.
- GPUI home: a local preferences store owned by the app, which survives a connection reset.

**L9. Sound playback** — **keep** the sounds SPEC names, **drop** the browser's gesture-before-playback rule

- Keep: SPEC, "Status": "Each client can play distinct sounds when a session starts waiting, when a new agent approval
  request arrives, and when an agent goes from running to idle", with switches "remembered per device". Code:
  `ui/assets/sounds.js` (event choice, notes, Web Audio player). Tests: `sounds.spec.ts`. GPUI home: a small platform
  audio player beside the UI layers, fed by the session list's status changes.
- Drop: the browser's autoplay policy. `ui/assets/sounds.js:58-117` creates the audio context, tries "desktop autoplay"
  and resumes it on the first trusted pointer, click or key press; a native app has no such gate.

## M. Terminal plumbing the browser supplies

**M1. The platform paste keys reach the terminal as a paste (Cmd+V on macOS; Ctrl+Shift+V and Shift+Insert in Linux
browsers)** — **keep**

- SPEC, "Terminal experience": "A user-initiated paste intentionally sends the pasted content to the selected terminal".
  Code: the browser's `paste` event on the hidden input (`ui/assets/grid-terminal.js:1549`,
  `ui/assets/terminal.js:2386`); the grid has no paste key of its own (`ui/assets/grid-input.js`).
- Tests: `terminal-attachments.spec.ts`.
- GPUI home: terminal + actions/keymap (choose the paste chords explicitly per platform).

**M2. Middle-click paste of the Linux primary selection into a terminal** — **keep** (decided 2026-11-13)

- `ui/assets/grid-terminal.js:1848-1853` moves the input under the pointer so a middle-button paste lands in it. SPEC:
  nothing.
- Today a terminal selection never sets the primary selection: completed selections and a program's OSC 52 writes go to
  the system clipboard only ("the browser has no primary selection", `ui/assets/terminal.js:4320-4330`).
- Decided: on Linux, selecting text in a terminal sets the primary selection and middle-click in a terminal pastes it.
  Terminal only; other fields do not take part. The selection side is new behavior; the paste side exists today.
- GPUI home: terminal.

## Decisions

Every ask entry was answered by the maintainer on 2026-11-13: Tab on macOS follows the system setting and Linux Tab
reaches every focusable control (C2); no reload command (L5); no resize grip (E4); no text drops into fields (H2);
right-to-left text gets only what GPUI gives (J6); no whole-interface zoom (L1); Linux middle-click paste in the
terminal only (M2).

Some of these decisions change visible behavior in ways SPEC.md does not state yet: a terminal selection setting the
Linux primary selection (M2), multi-line boxes growing to a cap and then scrolling (E4), Tab reaching every control on
Linux (C2), and text fields taking no text drops (H2). The area that owns each surface writes the decided behavior into
SPEC.md when it ports that surface.
