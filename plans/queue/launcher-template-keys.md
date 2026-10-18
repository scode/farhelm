# Template shortcuts in the launcher, and a template-only list on Cmd-J

Written against main at d526038e on 2026-10-18. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan. For context only: a plan written alongside this one, `repeat-quick-launch`,
changes how a template's session name resolves (a "base name / fixed name" kind). It does not change how template
shortcuts are dispatched. Keep this plan's changes to dispatch, the launcher's handling of template shortcuts, and the
Cmd-J list, so the two overlap as little as possible; if that plan has landed by the time you run, rebase onto it as
usual.

## The goal

Two TODO.md Near term entries, merged into one plan with the maintainer's agreement because both rework how template
shortcuts are dispatched and launched:

- "**Apply template hotkeys in the launcher.** While New, Clone, or Replace with is open, pressing a bound template
  hotkey should apply that template to the current launcher draft, exactly as accepting `tl:name` in the launcher's
  search does, instead of starting a session. This should work for the quick-launch bolt and number shortcuts, so
  pressing several hotkeys can stack their template edits before Launch."
- "**Open a template-only launcher with Cmd-J.** Make Cmd-J work like Cmd-K, but show only templates. Typing a template
  name and pressing Enter should launch it, so `Cmd-J, foo, Enter` starts the `foo` template."

Today every template shortcut has its own keyboard listener with its own copy of the chord test and its own copy of the
"a visible modal keeps the keyboard" check: the ⚡ chord is an inline listener in `crates/farhelm-ui/src/lib.rs`
(`install_quick_launch_shortcut`), the number keys are `crates/farhelm-ui/assets/template-number-shortcuts.js`, and
sequences are `crates/farhelm-ui/assets/launch-sequence.js`. The displayed chord labels are separate constants again
(`QUICK_LAUNCH_CHORD` and `number_chord` in `crates/farhelm-ui/src/list/templates.rs`, `PREFIX_CHORD` in
`crates/farhelm-ui/src/launch_sequence.rs`). All three listeners return without consuming the key whenever any visible
`[role=dialog][aria-modal=true]` exists, and the Rust triggers in `crates/farhelm-ui/src/list/view.rs` are disabled
while the launcher is shown, so inside the launcher nothing happens and Cmd/Ctrl+Shift+P reaches the browser (Print, or
a private window in Firefox). This plan first consolidates those into one definition and one dispatch path, then adds
the launcher behavior and Cmd-J on top of it.

### Acceptance criteria

One definition, one dispatch path (the maintainer's requirement, first-class):

- Every template shortcut (the ⚡ chord, the number chords, the sequence prefix and the keys inside a sequence, and
  Cmd-J) is defined in exactly one place: which physical key, which modifier scheme (the build's Mac-desktop choice for
  numbers and the prefix, the operating system's choice for ⚡ and Cmd-J, exactly as today), and how it is matched. The
  keyboard listener and every label the interface shows for these keys (Templates' hints and tooltips, the sidebar
  Templates button tooltip, the sequence pill, the quick switcher's Quick launch row, the Cmd-J rows) are derived from
  that one definition.
- One context function decides, for every template shortcut and for Cmd-J, which context a key press is in: no modal
  ("page": start a session), the hidden launcher of a quick launch still being prepared ("preparing"), the launcher as
  the only visible modal ("launcher": apply to the draft), or any other modal ("yield", as today). Each action maps
  those contexts in one place; the dispatch consults the same function rather than its own copy (today `view.rs`'s
  `quick_launch` callback carries a fourth copy of the modal check after its fresh read, which goes). One Rust dispatch
  path takes an ordered list of template references, reads preferences and templates fresh, resolves them through one
  resolver, and routes the result by context. The ⚡ chord, the number chords, sequences, the quick switcher's Quick
  launch row and Cmd-J all go through it; there is no second launch path and no second apply path.
- Concrete checks the reviewer applies to every PR from the first one on, and to the stack tip:
  - Thought experiment: list every line that must change to (a) move the sequence prefix from X to another letter, (b)
    move the ⚡ chord to another letter, (c) change the number keys' modifiers, (d) turn one of these keys into a user
    setting, (e) change which dialogs let template shortcuts through. The answer must be one entry in the shared
    definition (for d, the one place that hands the definition to the listener and the labels; for e, the one context
    rule), plus prose in SPEC/docs and tests that name keys literally. Anything else is a finding.
  - A JS unit test runs the shared classifier and context rule against a non-default definition (every template shortcut
    moved to a different key) and shows that every action, in both the page and the launcher context, follows it. A Rust
    unit test shows that the displayed labels follow the definition.
  - `grep -rn --exclude-dir=vendor -E 'KeyP|KeyX|KeyJ|Digit' crates/farhelm-ui/src crates/farhelm-ui/assets` finds the
    template shortcuts' key codes only in the shared definition (unrelated shortcuts such as Cmd-K's `KeyK`, Rename and
    Delete excepted). At d526038e it finds them in `template-number-shortcuts.js`, `lib.rs` and `launch-sequence.js`.
  - Only one code path performs the fresh preference-and-template read for template shortcuts, and only one function
    turns references into template definitions.

Template shortcuts inside the launcher:

- While New, Clone or Replace with is open and visible, and it is the only visible modal, the ⚡ chord, each number
  chord and each completed sequence (prefix, keys, Enter) apply their template(s) to the open draft, exactly as
  accepting `tl:name` in the launcher's search does, instead of starting a session. Nothing launches; Launch stays the
  user's action. Several presses stack, a later template winning on fields both set (SPEC.md's `tl:` stacking rule).
- No press is lost: presses sent back to back, faster than the fresh read completes, all apply, in press order. (Today
  `quick_launch` returns at once while a read is pending and the triggers are disabled, which would drop the second
  press.) Each press, or each completed sequence, is its own refusal unit.
- A template that does not fit is refused whole, with its message where `tl:` shows refusals; in Replace with that
  includes a template that sets a host. Earlier presses stay applied.
- Focus stays where it was: applying a template from a shortcut does not move focus. If the template removes the focused
  control (switching the launch kind unmounts the other tab's fields), focus goes where the launcher's own
  `focus_composer_surface` puts it.
- The keys are consumed in the launcher (the browser no longer gets Cmd/Ctrl+Shift+P there), held keys apply once,
  composition input is ignored, as for the existing shortcuts.
- In the launcher's own text fields (search, name, folder, command and the rest), the sequence prefix (Cmd+X in the Mac
  desktop app, Ctrl+Shift+X in browsers) starts a sequence only when no text is selected in the focused field. With a
  selection it keeps its normal meaning and is not consumed, so Cmd+X cuts as usual (Cut does nothing without a
  selection anyway). Text fields anywhere else keep the prefix's normal meaning, as SPEC.md says today. Once a sequence
  is armed, its keys are consumed as today, so bare digits typed in a launcher field queue templates instead of typing.
- Any other modal keeps the keyboard exactly as today: Templates, Settings, Rename, Restart with, the quick switcher,
  and any `role=dialog aria-modal=true` element shown over the launcher. The YOLO question is not such a modal (it is an
  inline `role=alertdialog` inside the launcher form): a shortcut applies while it shows, exactly as accepting `tl:name`
  does, and an edit that changes the draft retires the question by the existing rule. The hidden launcher of a quick
  launch still being prepared keeps today's behavior: the keys are consumed and do nothing.
- An unbound number key, no ⚡ mark, or a failed preference/template read shows a message in the same place as a `tl:`
  refusal and changes nothing; it does not open Templates over the launcher. While a launch from the launcher is in
  flight, or while its "save as template" panel is open or saving, a shortcut is consumed and changes nothing; log
  whether it also shows a message.

Cmd-J:

- Cmd+J on a Mac, Ctrl+Shift+J elsewhere (the same scheme as Cmd-K, matched as `lib.rs`'s quick-switcher listener does)
  opens a list of templates that works like the quick switcher but shows only templates: typing filters them by name the
  way the switcher's `tl:` search does (no `tl:` needed), each row shows the template's name and summary as the
  switcher's `tl:` rows do, plus its ⚡ chord or number chord when it has one.
- Enter launches the template whose name equals the trimmed typed text exactly (case-sensitively; names are unique that
  way) if there is one, otherwise the highlighted row. Enter with no match does nothing. So `Cmd-J, foo, Enter` starts
  the `foo` template. An exact name match is always the highlighted row: whenever the typed text changes and a name
  equals it, the highlight moves to that row, so what is highlighted is what Enter launches (typing `foo` over `afoo`
  and `foo` highlights `foo`, not `afoo`). Arrowing away from it afterwards moves the highlight, and Enter then launches
  the row the user arrowed to.
- Launching goes through the same quick-launch path as ⚡ and the number keys, by name: fresh read of templates and
  preferences, the same defaults, validation, host admission and YOLO question, and the same naming dialog after
  success. A template that cannot launch on its own (no agent or command) opens New prefilled, as a quick launch does.
- Cmd-J never reaches the terminal's program, and like Cmd-K it does nothing while any modal is open or before sign-in
  and preferences loading finish. In particular, while New, Clone or Replace with is open (or the hidden launcher of a
  quick launch is being prepared), Cmd-J does nothing, as Cmd-K does there today.
- The executor finds out which browsers keep the chord for themselves and documents the gaps in SPEC.md and the docs
  pages. The desktop app is the guaranteed target.

Both:

- SPEC.md, SPEC_impl.md and the affected docs pages (`website/src/content/docs/docs/using/launch-templates.mdx`,
  `session-list.mdx`, `start-a-session.mdx`) describe the new behavior; each `feat` PR carries a changelog fragment.
- The last code PR removes both TODO.md entries this plan covers.

## Requirement sources

**The user's decisions (2026-10-18), binding:**

- While New, Clone or Replace with is open, the ⚡ quick-launch shortcut, the number shortcuts and the Cmd/Ctrl+Shift+X
  sequences each apply their template(s) to the open draft, exactly as accepting `tl:name` in the launcher's search
  does, instead of starting a session. Several presses stack: a later template wins on fields both set.
- A template that doesn't fit is refused whole, with the message where `tl:` shows it; in Replace with that includes a
  template that sets a host.
- Focus stays where it was. Other dialogs still keep the keyboard, as today. The launcher no longer lets
  Cmd/Ctrl+Shift+P fall through to the browser.
- In the maintainer's words: "Make sure it's planned to be implemented in such a way that we don't just duplicate code.
  If in the future we change something like which key is triggering the emacs-style prefix flow, or we're making it a
  setting so you can pick the key, or we change some behavior, that should always naturally, because of how the
  implementation is structured and designed, mean that it changes all of these things, as opposed to the agent having to
  discover that it's duplicated all over the place." One shared definition of the template shortcuts (keys, matching,
  the modal/launcher context rules) and one dispatch path, used by every shortcut in both contexts (closed launcher:
  quick launch; open launcher: apply to the draft) and by Cmd-J where it overlaps. Consolidate first, then add the new
  behavior.
- Cmd-J works like Cmd-K but shows only templates. Typing a template name and pressing Enter launches it. Keys: Cmd+J on
  a Mac, Ctrl+Shift+J elsewhere, matching Cmd-K's scheme. Browsers may keep the key (the maintainer's understanding:
  Ctrl+Shift+J opens Chrome/Edge's DevTools console and Firefox's Browser Console, and Cmd+J opens Chrome's Downloads on
  a Mac; verify rather than copy this). Find out which browsers let the page have it and document the gaps; the desktop
  app is the guaranteed target.
- The Cmd-J list shows templates the way Cmd-K's `tl:` list does (name plus summary), plus each template's ⚡ or number
  key when it has one. Enter launches the exact name match if there is one, otherwise the highlighted row; Enter with no
  match does nothing. Later (2026-10-18): "an exact name match is always the highlighted row, so what is highlighted is
  what Enter launches." A template that can't launch on its own opens New prefilled, as a quick launch does. The naming
  dialog behaves as for any other quick launch. Launching by name uses a by-name template reference through the same
  quick-launch path (fresh read, auto-launch flow); no second launch path.
- Later the same day (2026-10-18), answering the planner's open questions:
  - Inside the launcher's own fields (search, name, folder and the rest), the sequence prefix starts a sequence only
    when no text is selected in the focused field; with a selection, Cmd+X (the Mac desktop app's prefix) cuts as usual,
    since Cut does nothing without a selection anyway. Browsers' Ctrl+Shift+X follows the same rule in launcher fields.
    Text fields elsewhere keep the prefix's normal meaning. (This corrects an earlier answer the same day, which had
    been asked with the prefix wrongly described as Cmd/Ctrl+Shift+X on every client.)
  - Cmd-J while New, Clone or Replace with is open does nothing, as Cmd-K does there today.
  - In Cmd-J, an exact name match is always the highlighted row, so what is highlighted is what Enter launches.
- Review gate: a fresh-context gpt-6.1-sol agent at high effort. No-workhorse mode.

**Binding repository constraints:**

- SPEC.md, Creation: "Launch templates" (applying, stacking, refusal of a template that does not fit or sets a field the
  dialog holds fixed, Restart with not offering templates), "Quick launch" (fresh reads, the naming dialog, the New
  fallback for incomplete templates, refusals and the YOLO question, "visible modals keep the keyboard"), "Template
  number shortcuts" and "Quick-launch sequences" (including that the prefix keeps its normal meaning in a text field,
  which the decision above narrows for the launcher's own fields to "only when text is selected"; the pill, Escape and
  cancellation). The keyboard paragraphs near Cmd+K ("Cmd+K on macOS, Ctrl+Shift+K elsewhere, opens a quick switcher…",
  and the terminal paragraph saying the switcher chord never reaches the program).
- SPEC_impl.md: the quick switcher paragraphs (listener, DOM-removal handoff, template snapshot, `tl:` delegation to the
  launcher's search helpers), the number shortcut, quick launch and auto-submission paragraphs, and "Launch templates"
  (ordered references resolved from one fresh read, one replay per effect pass, a refusal stopping the remaining replay,
  the launch-sequence asset owning only the armed flag and a pure classifier). Update these where the code changes;
  where a decision above changes SPEC.md, the PR changes SPEC.md to match and says so.
- Root `AGENTS.md`: Changelog fragments, Tests and validation (`.agents/test-authoring.md` for test changes, the
  test-sleep check), Talking to the user. `website/AGENTS.md` before editing the docs pages (unattended: no preview
  server; name the changed pages in the report). `.agents/validation.md` before choosing checks.

**Planner proposals (your implementation choices to start from; each has its reason):**

- The shared definition is a Rust value (in `crates/farhelm-ui/src/launch_sequence.rs` or a renamed successor module
  that takes over its role) listing each template shortcut's key code and modifier scheme. Rust derives the displayed
  labels from it and hands it, serialized, to the one JS installer call. Reason: the labels live in Rust and the
  listener lives in JS, so the definition has to reach both from one place. Passing it as data also keeps the listener
  free of key literals, so a later "pick your key" setting would change where the data comes from rather than the
  listener; do not build a live-update path for it now (the installers run once). The ⚡ and Cmd-J labels stay dual-form
  ("⌘P / Ctrl+Shift+P" today), because their scheme follows the operating system, which the Rust build cannot see in a
  browser.
- One asset (for example `crates/farhelm-ui/assets/template-shortcuts.js`, replacing `template-number-shortcuts.js`,
  `launch-sequence.js` and the inline ⚡ listener) holds one capture listener, one pure classifier that takes the
  definition as an argument, and one context function. It keeps today's install position (after the composition guard,
  before the other command listeners) and the sequence's `stopImmediatePropagation`. The JS unit tests for the two old
  assets merge into one test file for the new one. Reason: one listener removes the three copies of the chord and modal
  checks and the ordering dependency between listeners. Keep the hidden trigger buttons' class names unless the relay
  set itself changes; several browser specs select them.
- The context function, judged by visible `[role=dialog][aria-modal=true]` elements as the template shortcuts are today:
  any visible modal other than the launcher form is "yield"; the launcher form (`.create-session-form`, which already
  carries those attributes; mark it with a data attribute if that reads better than its class) as the only visible modal
  is "launcher"; no visible modal while the launcher form is present but hidden (its `hidden` attribute during
  quick-launch preparation) is "preparing"; otherwise "page". The template shortcuts consume and act in "page" and
  "launcher", consume and do nothing in "preparing", and return without consuming in "yield". Cmd-J acts only in "page".
  The asset exposes the function on `window` so the Rust dispatch evaluates the same function, at its start and again
  after its fresh read, in place of the inline check `quick_launch` has today. The sequence's existing cancellation on
  modal focus and on a modal opening stays, keyed to the same function, so a sequence started in the launcher is
  cancelled when another dialog opens over it. Reason: one function answers both "may the listener take this key" and
  "where does the dispatch route it", which is what thought experiment (e) checks; the list has no Rust signal for the
  form's reveal state (it is private to the form), and the DOM already answers it.
- On the Rust side, the sequence queue's `Option<String>` (digit or bolt) becomes a small reference type (⚡, number
  key, and in the Cmd-J PR a template name), and the resolution in `view.rs`'s `quick_launch` callback moves into one
  pure resolver with unit tests. The `quick_launch` callback becomes the one dispatch, routing by the context function:
  "page" to today's quick launch, "launcher" to the open form, anything else dropped. The relays' `disabled` conditions
  stop keying on `show_create()` for the launcher route. Reason: the callback already is the shared entry for ⚡, number
  keys, sequences and the switcher's Quick launch row; extending it is smaller than adding a parallel path.
- The launcher route does not bump `opening_generation`: inside an open form that bump clears a visible
  permission-escalation notice and feeds the quick-submission fences. It records the current generation and drops the
  result if the generation changed during the read (closing the launcher and every new opening already bump it).
- The launcher route must not lose presses (see the acceptance criteria). Either a press that arrives while a launcher
  read is pending joins that read, resolved with it in press order, or presses are serialized, each starting its read
  when the previous one finishes. Pick one, log it, and keep each press (or sequence) its own refusal unit. The browser
  test sends presses back to back, without waiting for the first to apply.
- The open launcher applies the resolved definitions through the form's existing replay machinery (the
  `pending_initial_templates` queue drained by the effect near `create_form.rs`'s initial replay, and `accept_search`
  with a captured definition). That queue is seeded only at mount from a prop (and Clone/Replace with pass it empty), so
  the list needs a channel into an already open form: a list-owned request signal passed as a prop, which the form
  drains into its queue. Each queue entry carries its origin, because four things must not happen for a shortcut's
  entries: the "loading session setup…" state (it replaces the form and would lose focus), `accept_search`'s closing
  `focus_composer_surface`, its clearing of the search text and list, and the partial-template hint. Shortcut entries
  still wait for the readiness the initial replay waits for (templates, catalog and hosts read) and replay one template
  per effect pass. Today a refusal clears the whole queue; with more than one press queued it clears only the rest of
  that press or sequence. When `accept_search` refuses because a launch is in flight, a shortcut's entry is dropped, not
  left queued to apply at some later rerun. Reason: that path already gives "exactly as accepting `tl:name`" including
  stacking and refusal; only its opening-specific side effects differ, which is an extension, not a second apply path.
- The launcher's unbound-key, no-mark and read-failure messages use the form's existing `template_error` slot, where
  `tl:` refusals appear. Reason: the decision puts refusals there, and opening Templates over the launcher would need a
  second modal the current flow never shows.
- In the launcher context the sequence pill says it applies rather than launches ("Enter apply · Esc cancel", and the
  empty-queue text likewise) and must be visible while the launcher is open.
- The prefix's text-field exception is decided in the same one place as every other context rule: in "page" any ordinary
  text editor keeps the prefix, as today; in "launcher" only an editor holding a non-empty selection keeps it
  (`selectionStart !== selectionEnd` for inputs and textareas, a non-collapsed selection for an editable element). The
  classifier receives that answer rather than testing for editors itself.
- Cmd-J reuses the quick switcher component (`crates/farhelm-ui/src/list/quick_switcher.rs`) in a templates-only mode,
  opened through the same hidden trigger and DOM-removal handoff as Cmd-K (the trigger carries the mode). Its filter is
  the launcher's own template matcher (`template_search_results` in `crates/farhelm-ui/src/launch_composer.rs`, called
  with the `tl:` scope, or a small extraction of its matching step), so `tl:` and Cmd-J cannot drift apart. A new
  `SwitcherPick` variant carries the picked name to `finish_switcher`, which calls the one dispatch with a by-name
  reference. Clicking a row launches, as Enter on it does. The row rendering is shared with the `tl:` rows; Cmd-K's own
  `tl:` rows stay as they are. The key badges use the labels derived from the shared definition. Reason: the switcher
  already owns the snapshot, filtering, keyboard handling, modal isolation and focus restore that "works like Cmd-K"
  asks for.
- A by-name reference that no longer resolves at the fresh read (the template was renamed or deleted in another window
  between the Cmd-J snapshot and Enter) uses an existing fallback rather than a new hint mechanism: open New with the
  existing preparation error slot (`initial_create_error`) naming the missing template, or open Templates without a
  hint. Pick one and log it.
- Cmd-J's chord lives in the shared definition and it maps the shared contexts as above. Whether Cmd-K's own listener
  also moves into the shared asset is your call (log it), as long as Cmd-K and Cmd-J open the switcher through one
  opener.
- Browser reservations of Cmd-J: established from the browsers' own shortcut documentation (a Playwright key press does
  not show what a real browser keeps), recorded in SPEC.md's existing "a browser that reserves the chord keeps its
  behavior" style and in the docs, naming each known gap.

## Outline

A linear stack, bite-sized; merge 01 and 02 if 02 turns out trivially small.

1. `refactor`: one template shortcut definition and one keyboard listener. The Rust definition and derived labels, the
   merged asset with its pure classifier and context function, the merged JS unit tests including the non-default
   definition test, the Rust label test. No behavior change: the existing browser tests pass unchanged. SPEC_impl.md's
   listener paragraphs updated. No changelog fragment.
2. `refactor`: one Rust dispatch path. The reference type, the pure resolver with unit tests, the dispatch consulting
   the shared context function instead of its inline modal check, the sequence queue and the ⚡/number/sequence/
   switcher-row relays all calling the one dispatch. No behavior change, no fragment.
3. `feat`: template shortcuts apply to the open launcher. The "launcher" context's actions, routing to the launcher, the
   list-to-form channel, the replay extension (origin marker: no loading state, focus untouched, search untouched, no
   partial-template hint), press ordering, the generation fence without a bump, the messages, the pill wording and
   visibility, the prefix in the launcher's own text fields (a sequence without a selection, normal meaning with one),
   SPEC.md (Quick launch, number shortcuts and sequences: the launcher exception to "visible modals keep the keyboard",
   and the prefix's text-field rule for the launcher's own fields, in both the Creation paragraph and the keyboard
   paragraph near Cmd+K that says text fields keep their Cut or text-direction key), SPEC_impl.md, the docs pages, a
   changelog fragment. Browser tests in `e2e/tests/quick-launch.spec.ts` (or a new spec beside it), on both engines; the
   first one also proves that a hidden trigger outside the launcher still dispatches while the launcher's modal
   isolation makes it inert: with New open, ⚡ then a number chord sent back to back stack onto the draft with the later
   template winning, nothing is created, focus stays on the control that had it, and the chord's `defaultPrevented` is
   true; a sequence started with focus on a launcher button applies its templates in order at Enter; in a launcher text
   field with no text selected the prefix is consumed and starts a sequence that applies at Enter; in the same field
   with text selected the prefix is not consumed (`defaultPrevented` false), no sequence starts and the field keeps its
   text for the browser to act on; the prefix still keeps its normal meaning in a text field outside the launcher (the
   existing fixture test). Cmd+X's Cut in the Mac desktop app cannot run in the browser suite, so a JS unit test with
   the Mac-desktop definition shows the prefix passing through untouched in a launcher field with a selection (Cut
   happens) and starting a sequence without one; a refused template leaves the draft unchanged with the message where
   `tl:` shows it, including a host-setting template in Replace with; Clone applies too; the Templates dialog, and a
   programmatic `role=dialog aria-modal=true` fixture over the launcher (as the sequence cancellation test already
   mounts one), still keep the keyboard (`prevented: false`); an unbound key in the launcher shows its message and opens
   nothing.
4. `feat`: Cmd-J template list. The definition entry, the switcher's templates-only mode, by-name references, key
   badges, Enter's exact-match rule, the launch through the one dispatch, SPEC.md (a Cmd-J paragraph beside Cmd-K's, the
   terminal paragraph, the naming-dialog list in Quick launch, the browser gaps), SPEC_impl.md, the docs pages, a
   changelog fragment, and the removal of both TODO.md entries. Browser tests in `e2e/tests/quick-switcher.spec.ts` (or
   beside it): Cmd-J from a focused terminal lists only templates with summaries and keys; `foo` then Enter launches
   `foo` through the quick-launch path (one create, naming dialog per the preference); typing `foo` with an earlier
   substring match listed highlights `foo` and Enter launches it; no match makes Enter do nothing; a partial template
   opens New prefilled; Cmd-J does nothing with New open, with another modal open, or while the hidden launcher of a
   quick launch is being prepared; Escape restores focus. Check `e2e/tests/tooltip-coverage.spec.ts` covers the new
   rows.

### Validation

Choose per `.agents/validation.md`; at least, at the PRs where they apply: `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, `cargo check -p farhelm-ui --features desktop` (the Mac branches are
type-checked there), the JS unit harness in `crates/farhelm-ui/js-tests`, the `farhelm-ui` unit tests you touch through
the recorder, `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`,
`python3 releasing/check-changelog.py format`, `dprint check`, and the website build for the docs pages. On Chromium and
WebKit through the recorder, after `dx build`: `quick-launch.spec.ts`, `quick-switcher.spec.ts`, the `tl:` and menu
tests in `templates.spec.ts`, `ime-composition.spec.ts` (composition must still be ignored), and the specs that use the
hidden triggers (`github-checkouts.spec.ts`, `approval-layout.spec.ts`, `tooltip-coverage.spec.ts`). PRs 01 and 02 are
refactors, so the existing specs above are their main evidence. Not the full suite unless a concrete risk needs it. The
report says that Mac-desktop chords are covered only by the JS unit tests and the desktop type-check, and that browser
reservations come from documentation, not tests.

## Fallbacks

- If a single listener cannot keep an ordering the existing tests depend on, keep more than one listener, but every one
  of them takes its keys, matching and context from the one shared definition and context function. The structural
  checks above still hold. Log it.
- If the desktop app itself (its webview on macOS or Linux) keeps Cmd+J or Ctrl+Shift+J from the page, block: which key
  Cmd-J moves to is the maintainer's choice. A browser that keeps it is documented, not a block.
- If applying in the launcher cannot avoid moving focus or showing the loading state without a second apply path, that
  is a departure for the scope reassessment, not a reason to add a parallel path.

Examples of departures that need the scope reassessment: a general keymap or shortcut registry beyond the template
shortcuts and Cmd-J, persisting the definition anywhere, a second template-application path beside `accept_search`, a
new dialog component for Cmd-J instead of a switcher mode, changing the sequence prefix's text-field exception beyond
the selection rule for the launcher's own fields.

Decisions to log: where the definition lives and how it reaches the listener; the context function's exact shape;
merging or serializing presses during a pending read; the list-to-form channel and how a shortcut's replay differs from
the opening replay; the launcher messages' wording, and whether a press refused while a launch is in flight or the
save-as-template panel is open says anything; the pill wording; Cmd-J's dialog name and its fallback for a template gone
by Enter; whether Cmd-K's listener moved; the browser findings for Cmd-J and their sources.

Things the report must say plainly: Mac-desktop chords, Cmd+X's Cut in a launcher field included, are covered only by
the JS unit tests and the desktop type-check.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md` (Files outside the repository): this plan's working log is
`farhelm-plan-launcher-template-keys-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md` (Executing one plan, step 7)
before any work.

### No-workhorse

The user forbids sending any unit of your own decomposition of this work to a weaker model or a lower effort than your
own. The review gate and the scope reassessment review are not units of that decomposition: they keep the model and
effort this file demands for them. The user also forbids invoking, loading or following `scode-galaxy-brain` or
`scode-model-routing` at any point; if galaxy-brain is already active when you start, this file is the user's express
instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering whenever they are likely to finish the work
sooner without costing quality: independent investigations run in parallel, or implementation split across agents.
Whether to use them is your call. Concurrent agents that might otherwise conflict can run in separate jj workspaces
whose results you merge back; you still own the stack and every PR. A sub-agent whose model is not available as a native
sub-agent type, the reviewer included, is launched through `$scode-harness-shellout`. The scope reassessment review runs
at your own model and effort. The sub-agents `plans/AGENTS.md` (Sub-agents this file requires) requires of every plan
(the resume check, and the cold reads of a blocked question and of the report) run as that file says.

### Resource watchdog

Immediately after the resume protocol, start a resource watchdog as a background process (not an agent) and keep it
running for the whole run. Every 60 seconds it samples free space on the filesystems holding the checkout, the agent
scratch directory and `/tmp`, plus available memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat`
and `sysctl` on macOS). It writes each sample to a private heartbeat/status file in the scratch directory, and it emits
a notification when any of these happens:

- a watched filesystem drops under 10% or under 5 GB free, or available memory drops under 10%, whichever comes first;
- the number keeps falling;
- the condition recovers;
- the monitor itself fails.

In Claude Code a notification is a stdout line of a monitor started with the Monitor tool; elsewhere, the harness's
equivalent. A file update alone is not a notification.

Before relying on the watchdog:

- verify delivery with a harmless synthetic alert;
- verify failure detection by killing a throwaway monitor and confirming you are told;
- if you omit periodic heartbeat checks, also stall a throwaway monitor without killing it and confirm you are notified
  within two sample intervals, since a monitor cannot detect its own sampling loop hanging.

If any of these notifications is unavailable or unverified, say so in the log and check the status file at least once a
minute.

On every path, check heartbeat freshness before each new build, test run or review launch. A dead monitor, or a
heartbeat stale for two sample intervals, pauses new launches until monitoring is restored; restart a dead watchdog.

An alert is an instruction to act:

1. stop launching work;
2. remove build output and scratch you own;
3. wait for or stop the job most likely responsible;
4. resume only when the watchdog reports headroom.

Record the watchdog's handle, watched paths, status path and delivery mechanism in the log. Include its state in every
handoff note so a resumed session reconciles or restarts it, and stop it when the plan closes. Do not lengthen the
sampling interval to save turns.

### PR discipline

- Split the work into a linear stack of reviewable PRs, one reviewable commit, stable bookmark and draft PR each, using
  the `jjstack` skill. The outline above suggests the slicing; err on the side of bite-sized PRs.
- The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/.agents/executing.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/launcher-template-keys/<nn>-<short-name>` (for example `plan/launcher-template-keys/01-shortcut-definition`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, with changelog fragments per root `AGENTS.md` (Changelog
  fragments): a later PR that extends an earlier PR's still unreleased change edits that fragment rather than adding a
  second one.
- The last code PR removes both TODO.md entries this plan covers (each ends with this plan's `Plan:` reference, which
  the planning PR that queued this plan added). TODO.md is a shared bookkeeping file: on a rebase conflict there, keep
  both sides.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, Executing one plan, step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes yourself and address what it finds before moving on. The user
demands exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched
through `$scode-harness-shellout` when your harness cannot serve that model natively (that skill owns the launch
mechanics). The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name:

- the repository root (the executing checkout);
- the bookmark or commit range;
- a findings file in the scratch directory;
- the acceptance criteria: this file's goal, the user's decisions, and the outline;
- the structural checks under "One definition, one dispatch path" above, which the reviewer applies to the PR and
  reports on explicitly (the thought experiment's answer, the grep result, the two unit tests), even when it finds
  nothing.

When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim, as root
`AGENTS.md` requires.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review at your own model and effort:

- before implementing a substantial departure from the outline above (the plan-specific examples are under Fallbacks
  above; they are examples, not a blacklist);
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. Same-model
sub-agents you choose to run get bounded units within the outline; an unapproved new mechanism is outside their task,
and one discovered mid-unit is left unimplemented and reported back to you. Apply the same decision boundary as
Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate (a line number, a file name) is corrected to the truth; the plan-specific
agreed fallbacks are under Fallbacks above.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/.agents/executing.md` (Executing one plan, step 10) only for what
cannot proceed without the user.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of open draft PRs exists that together meets the acceptance criteria above,
each PR passed its review gate, and the last code PR removes both TODO.md entries. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md` (Executing
one plan, steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the
watchdog. Never edit `plans/` yourself.
