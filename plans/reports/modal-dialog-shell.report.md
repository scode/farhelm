## What this was about

Farhelm's modal dialogs each assembled their own keyboard trap, page isolation, initial focus and return of focus. Fixes
kept reaching one dialog while its siblings retained the same problem. This plan puts those rules behind one shared
dialog boundary, preserving each dialog's existing actions, request ordering and focus destinations.

The inventory found 17 modal surfaces: Rename and post-launch naming; Restart with; New, Clone and Replace with; the
quick switcher and template launcher; Settings; Feedback; Trash; Templates; Host Settings; host removal; host-operation
refusals; Add host; row setup; row uninstall; Welcome; SSH help; and native tunnel consent. New/Clone/ Replace with
share one surface, as do the switcher's two modes. All render their existing native div or form through the shared
boundary, without another wrapper. Anchored menus, the notification list and inline confirmations remain outside this
plan.

## Things you should know

The first seven PRs preserve behavior. The final three contain the authorized focus fixes, each with a changelog
fragment. Every modal now traps Tab and isolates the page behind it. Independent agent approval cards remain usable.
Hidden quick-launch preparation isolates without focusing hidden fields. Native controls retain their own interior Tab
and radio navigation; Rename keeps its specified empty-space-click exception.

Known closes and retirements release isolation before hiding the dialog or handing focus elsewhere. Unmount cleanup is a
fallback and does not return focus. The quick switcher still distinguishes cancellation, selecting the current session,
selecting another session and handing off to a launcher. Host dialogs retain delayed admission and their existing row,
Add and Settings return destinations. No modal stack or queue was introduced, and this does not convert the app to
native HTML dialog elements.

Scrollable bodies gain a named Tab stop only while their contents have no usable native Tab targets. Nine shared body
renderings use that rule; Templates applies it to its existing dialog root. Removed or disabled controls trigger
recovery without taking focus from a usable control or an approval answer. A delayed SSH help response therefore no
longer steals an approval answer's keyboard focus. Its review-queue item and index entry are removed in the fix PR. The
plan's TODO entry is removed in the last code PR.

The stack was carefully rebased onto main at `89c5b9b7ba74`. Main's intervening changes centralize launch-file cleanup
and workspace-trust admission in the servers; their request and callback contracts remain compatible with these focus
changes. The sole conflict was adjacent completed-plan entries in TODO; all completed entries stay removed. The modal
source and tests were preserved byte for byte. The seven concurrent launcher PRs were separately checked for overlap:
they change draft/request ownership rather than the modal focus islands. Their future landing still needs to retain both
sets of edits when resolving surrounding text.

## Open questions and possible follow-ups

Two existing findings remain in the review queue. In a very short window, an unavailable-session explanation may cover
sidebar controls used for recovery; that is a nonmodal overlay/layout issue. Host Settings may still clip its footer
Close button in a very short window; the header close and Escape remain available. Neither layout is changed here.

Native tunnel consent compiles and its accepted-save/source-retirement controller is tested. Browser tests cannot prove
the native webview's consent-dialog removal and Settings-restoration timing. That remains a validation gap. An existing
desktop test-helper Clippy complexity warning also remains; it is unrelated to this plan.

## The PRs

- [#2519](https://github.com/scode/farhelm/pull/2519/changes): shared dialog boundary and Restart with conversion.
- [#2524](https://github.com/scode/farhelm/pull/2524/changes): Settings, Feedback, Trash and Templates conversion.
- [#2525](https://github.com/scode/farhelm/pull/2525/changes): host-dialog conversion.
- [#2527](https://github.com/scode/farhelm/pull/2527/changes): Welcome, SSH help and native tunnel-consent conversion.
- [#2529](https://github.com/scode/farhelm/pull/2529/changes): quick-switcher conversion and close reasons.
- [#2531](https://github.com/scode/farhelm/pull/2531/changes): Rename and naming conversion.
- [#2533](https://github.com/scode/farhelm/pull/2533/changes): New, Clone and Replace with conversion.
- [#2534](https://github.com/scode/farhelm/pull/2534/changes): common modal isolation and native Tab containment.
- [#2535](https://github.com/scode/farhelm/pull/2535/changes): release isolation at known close and retirement
  boundaries.
- [#2537](https://github.com/scode/farhelm/pull/2537/changes): conditional keyboard entry and lost-focus recovery.

All ten are draft PRs in one linear stack. This report does not claim that they have merged.

## Checks run, reused and skipped

Each strict recorded browser selection ran Chromium and WebKit with three configured worker slots, one private stack per
slot, no retries and pinned tmux 3.7c. The retained records have complete source/report identity, ordered output and
cleanup evidence. The following focused observations are reused after the content-preserving rebase; their covered
review units are identified by the original pushed revision. Later fix units supplement the earlier conversion checks.

| Covered unit/revision               | Recorded run                           | Observation and scope                                                                   |
| ----------------------------------- | -------------------------------------- | --------------------------------------------------------------------------------------- |
| 1, `ea924f315c24`                   | `ae717708-875b-4b76-a40f-0387279a41b4` | 20 passes, 10 per engine: Restart with focus, busy/refusal and return paths.            |
| 2, `b38fc08c634d`                   | `b4d64af5-4141-40c0-a4ac-bf6b74cbcd3c` | 32 passes, 16 per engine: Settings, Feedback, Trash and Templates.                      |
| 3, `2dd5abd417b2`                   | `4021e51f-3715-48ec-91db-365edf616c4b` | 29 passes and one Chromium timeout before the setup dialog opened; command failed.      |
| 3, same source                      | `265b1679-335f-4300-b23c-102a9b98bd0b` | Two exact Chromium repetitions passed, covering the missing setup path.                 |
| 4, `3f2ec341bff9`                   | `e2f837aa-ac99-413d-b883-196df0b900bd` | 42 passes, 21 per engine: help, Welcome, nested restoration and delayed host questions. |
| 5, `d17381d93918`                   | `476fa2e6-4848-4d1a-94a8-6a18b341aeed` | 16 passes, 8 per engine: switcher close reasons and launcher handoffs.                  |
| 6, `c2f82e877a05`                   | `b5fc0bf7-6b03-4646-b2e9-43bc089f355a` | 16 passes, 8 per engine: Rename and post-launch naming.                                 |
| 7, `12f7753b16e2`                   | `cca233e2-e953-4909-8fa9-8adb42601fa7` | 40 passes, 20 per engine: launcher mount/reveal, native form behavior and handoffs.     |
| 8, `3a4d9d787c25`                   | `f551e40d-80d1-4efe-97fc-934db6ba12c9` | 34 passes and two new-test locator timeouts, one per engine; command failed.            |
| 8, corrected fixture                | `90ca85fe-3c59-4959-9f61-cdb51f0e1ce6` | Two exact passes, one per engine; unchanged other 17 cases per engine reused.           |
| 9, `25025ddacf20`                   | `33a3f6e9-98f4-4e37-8be4-172297dd36bf` | 10 passes, 5 per engine: switcher and real host-dialog release-before-removal ordering. |
| 10, corrected source `4d620977`     | `aab2bace-b71b-4276-aa43-1e5566b55190` | 34 passes and four failures across two fixtures, both engines; command failed.          |
| 10, final fixture source `a010277d` | `60288e30-c1fe-4c6d-a103-13cabb0f3b19` | Four exact passes, two per engine; unchanged other 17 cases per engine reused.          |

All three failed browser records remain retained. The host setup failure occurred while a row-menu item detached before
opening the modal; its cause is unproven. The containment test used the wrong accessible locator, corrected to the real
combobox with a readiness premise. The final two fixture failures came from freezing animation frames along with
Feedback's close timer and returning from a canceled Playwright route without completing its route action. Their fixes
preserve native keyboard, lifecycle and timer assertions. Later passes do not turn those failed commands into passes.
Same-session fixture corrections were not added to the latent-flake log.

The final UI source passed 773 desktop-feature library unit tests, zero skips, through recorded nextest 0.9.148 with
four slots and zero retries: `d51ccfc6-ff73-47aa-9acc-328e6aef1a2f`. It covers the corrected unit-10 Rust source; the
subsequent changes are browser fixtures and the audited upstream server rebase. The exact native accepted-save/source-
retirement test also passed in `fb7147d6-4ee6-464c-8332-6748a468a201` and, after the explicit-release change, in
`99697483-ef1a-40d6-8c56-bb8c3102eb3e`. These are controller/unit evidence, not native DOM timing evidence.

The final Rust source passed `cargo fmt --all -- --check`, web-target UI Clippy, default-feature UI all-targets Clippy,
and desktop-feature UI library Clippy, all with warnings denied:

```sh
cargo fmt --all -- --check
cargo clippy -p farhelm-ui --target wasm32-unknown-unknown --features web -- -D warnings
cargo clippy -p farhelm-ui --all-targets -- -D warnings
cargo clippy -p farhelm-ui --features desktop --lib -- -D warnings
```

The attempted `cargo clippy -p farhelm-ui --features desktop --all-targets -- -D warnings` command failed on an
unchanged type-complexity warning in the approval test renderer; its signature was checked against the parent revision.
It is not reported as a pass. Release web builds passed at each unit; the final matching bundle is reused because the
rebase changes no UI source or web dependencies. The isolated test-delay checker inspected 314 calls with zero missing
rationales. Changelog layout, changed Markdown formatting and instruction-link checks passed.

After the rebase, `cargo build` rebuilt the servers and fixtures successfully, and formatting and instruction-link
checks passed again. The matching web bundle was reused. Recorded run `d08b7434-aa13-49d5-a2d2-90ddb5012cf4` on combined
tip `ae702581dede` passed four executions, two per engine: the quick switcher's real direct-create handoff and Restart
with's held/refused focus path. The restart response is intercepted, so that case proves focus and terminal reattachment
rather than execution of the helm's trust resolver. The direct-create case reaches the real backend.

Full-workspace runtime tests, installer/provisioning distribution gates, desktop process smoke and the full browser
suite were skipped: focused dialog selections and UI units cover the identified changes, and the audited rebase does not
justify repeating unrelated suites. Asset-JavaScript unit tests were skipped because no asset JavaScript changed. No CI
run was requested or implied; this repository runs CI only on demand. The other build/source commands were:

```sh
cargo build
dx build --package farhelm-ui --platform web --release
python -B scripts/check-test-sleeps.py
python3 releasing/check-changelog.py format
dprint check SPEC.md SPEC_impl.md TODO.md review_feedback_queue/INDEX.md
python3 scripts/check-agents-md.py
```

The web build ran from `crates/farhelm-ui/`; the delay check used the isolated pinned-parser interpreter documented by
the repository. The final changelog check covered 80 fragments.

### Recorded commands and selections

These are the exact portable child commands from the retained records; browser commands run from `e2e/`, Rust commands
from the checkout root. Each was invoked through `scripts/record-test-run.py` with the runner and substrate described
above. The exact Chromium repetition used generic recording; the other browser commands used `--runner playwright`, and
the Rust commands used `--runner nextest`.

Run `ae717708-875b-4b76-a40f-0387279a41b4`:

```sh
'npx' 'playwright' 'test' 'restart-with\.spec\.ts' '-g' 'gap focus and container Enter|YOLO restart-with requires confirmation inside|focus inside the dialog over a live terminal|focus inside the dialog when the selected tab|Tab from the open model list|Tab inside the dialog after a click|pulls stray focus back|page inert while open|first (effort|model) Enter keeps focus'
```

Run `b4d64af5-4141-40c0-a4ac-bf6b74cbcd3c`:

```sh
'npx' 'playwright' 'test' 'settings\.spec\.ts' 'feedback\.spec\.ts' 'trash-dialog\.spec\.ts' 'templates\.spec\.ts' 'claude-trust\.spec\.ts' 'restart-with\.spec\.ts' '-g' 'settings gap keeps focus|host removal container Enter|settings dismisses a row menu|send feedback sends exactly|a close queued behind send|trash list stays keyboard-scrollable|trash header and global confirmation|number popup closes when the dialog moves|unsaved departures support|model suggestions and phone departure|Claude trust hint preserves|gap focus and container Enter|Tab from the open model list'
```

Run `4021e51f-3715-48ec-91db-365edf616c4b`:

```sh
'npx' 'playwright' 'test' 'terminal-multihost\.spec\.ts' 'provisioning\.spec\.ts' '-g' 'host-settings-dialog-lifecycle|host-settings-backdrop-escape|remove-cancel:|Remove keeps its dialog open through a refused request:|delayed host dialogs wait for Settings:|uninstall and Update planning refusals open framed dialogs|queued Update refusals retain modal focus and isolation|confirmed uninstall refusal survives a vanished host row|host-setup-layout|cancel and a permanent answer on a host row'
```

Run `265b1679-335f-4300-b23c-102a9b98bd0b`:

```sh
'npx' 'playwright' 'test' '--project=chromium-provisioning' '-g' 'cancel and a permanent answer on a host row' '--repeat-each=2'
```

Run `e2f837aa-ac99-413d-b883-196df0b900bd`:

```sh
'npx' 'playwright' 'test' 'ssh-help\.spec\.ts' 'welcome\.spec\.ts' 'provisioning\.spec\.ts' 'restart-with\.spec\.ts' 'templates\.spec\.ts' '-g' 'ssh help|welcome dismissal by|welcome links, modal isolation|send feedback now records seen|help reopens welcome|probe sends no install paths, and a doubled submit|cancel stays live while discovery is pending|cancel and a setup answer in one burst|a refused ADD attempt reopens|a malformed accepted ADD closes|host-setup-layout|gap focus and container Enter|Tab from the open model list|number popup closes when the dialog moves by (scroll|resize)$'
```

Run `476fa2e6-4848-4d1a-94a8-6a18b341aeed`:

```sh
'npx' 'playwright' 'test' 'quick-switcher\.spec\.ts' 'quick-launch\.spec\.ts' 'template-list-shortcut\.spec\.ts' '-g' 'chord owns terminal input and restores focus|a busy pick leaves navigation|loading has no selection and a failed listing says so|pinned name action opens ordinary New|template pick waits for launcher inputs|quick switcher row launches through the fresh direct path|Cmd-J exact name launches'
```

Run `b5fc0bf7-6b03-4646-b2e9-43bc089f355a`:

```sh
'npx' 'playwright' 'test' 'terminal-replay-rename\.spec\.ts' 'modal-focus\.spec\.ts' 'session-action-shortcuts\.spec\.ts' 'quick-launch\.spec\.ts' '-g' 'held rename success|held rename refusal|rename-draft-survives-a-failed-read|rename survives a real committed row reorder|rename keeps focus in its field when the selected tab goes away|browser Rename chord edits|repeated naming selects the title'
```

Run `cca233e2-e953-4909-8fa9-8adb42601fa7`:

```sh
'npx' 'playwright' 'test' 'clone\.spec\.ts' 'launcher-enter\.spec\.ts' 'composer-word-search\.spec\.ts' 'launcher-tabs\.spec\.ts' 'quick-launch\.spec\.ts' 'templates\.spec\.ts' '-g' 'opening a clone focuses search|opening a clone from the keyboard|a deliberately entered name field|a closed opening.s held focus work|composer search survives a held terminal reveal|launcher gap keeps focus|launcher container Enter waits|launcher repeated Enter never launches|launcher action-button Enter keeps reset browse and cancel|launcher choice Enter leaves the YOLO|Tab from the open model list reaches|the arrow keys move between the launch-kind tabs|hidden preparation isolates keys|non-Mac quick launch owns input and submits once without showing New$|the naming preference off launches|quick launch reveals the ordinary YOLO question and retains its draft$|a quick preparation error survives|save as template captures the agent setup without launching$|launcher template save refuses a taken name|save as template keeps the draft on composing Escape'
```

Run `f551e40d-80d1-4efe-97fc-934db6ba12c9`:

```sh
'npx' 'playwright' 'test' 'modal-trap\.spec\.ts' 'terminal-replay-rename\.spec\.ts' 'modal-focus\.spec\.ts' 'restart-with\.spec\.ts' 'composer-word-search\.spec\.ts' 'quick-launch\.spec\.ts' 'templates\.spec\.ts' 'ssh-help\.spec\.ts' 'welcome\.spec\.ts' 'provisioning\.spec\.ts' '-g' 'switcher wraps search and close|ordinary New isolates the page|Rename wraps its textarea|busy Templates contains Tab|held rename success preserves an unrelated focus destination|rename keeps focus in its field when the selected tab goes away|restart with keeps Tab from the open model list|restart with keeps Tab inside the dialog after a click|restart with pulls stray focus|restart with makes the page inert|Tab from the open model list reaches|hidden preparation isolates keys|repeated naming selects the title|save as template captures the agent setup without launching$|add host opens ssh help for its draft|send feedback now records seen|delayed host dialogs wait for Settings: two plans$'
```

Run `90ca85fe-3c59-4959-9f61-cdb51f0e1ce6`:

```sh
'npx' 'playwright' 'test' 'modal-trap\.spec\.ts' '-g' 'ordinary New isolates the page and wraps its concrete command controls$'
```

Run `33a3f6e9-98f4-4e37-8be4-172297dd36bf`:

```sh
'npx' 'playwright' 'test' 'tests/(quick-switcher|provisioning)\.spec\.ts' '-g' 'the non-Mac chord owns terminal input and restores focus|offer retirement releases only its owner|host retirement releases before removal and preserves approval focus'
```

Run `aab2bace-b71b-4276-aa43-1e5566b55190`:

```sh
'npx' 'playwright' 'test' 'tests/(keyboard-scroll|templates|provisioning|terminal-multihost|trash-dialog)\.spec\.ts' '-g' 'sending feedback gains a body stop and refusal restores editor entry|templates busy root stays keyboard-scrollable at width (1100|390)|unstructured (setup|uninstall) plan keeps a named keyboard entry|host-setup-layout (phone|desktop) keeps answers and exact plan text|uninstall-remote-host: the plan names the kept data, confirming removes the host|SSH command arrival preserves an independent approval owner|SSH help closed before its reply leaves a reopened region independent|trash list stays keyboard-scrollable while every list action is disabled|host settings keep usable focus after a (refused destination|successful checkbox) save|sent feedback is keyboard-scrollable without changing the editing Tab order|SSH command arrival keeps usable focus from (body|dismiss|backdrop)|model suggestions and phone departure controls remain reachable|host settings automatic busy recovery remembers the commands checkbox'
```

Run `60288e30-c1fe-4c6d-a103-13cabb0f3b19`:

```sh
'npx' 'playwright' 'test' 'tests/keyboard-scroll\.spec\.ts' '-g' 'SSH help closed before its reply leaves a reopened region independent|sent feedback is keyboard-scrollable without changing the editing Tab order'
```

Run `d08b7434-aa13-49d5-a2d2-90ddb5012cf4`:

```sh
'npx' 'playwright' 'test' 'tests/(quick-launch|restart-with)\.spec\.ts' '-g' 'the quick switcher row launches through the fresh direct path|restart with keeps focus inside the dialog over a live terminal while pending and after refusal'
```

Run `d51ccfc6-ff73-47aa-9acc-328e6aef1a2f`:

```sh
'cargo' 'nextest' 'run' '-p' 'farhelm-ui' '--features' 'desktop' '--lib'
```

Run `fb7147d6-4ee6-464c-8332-6748a468a201`:

```sh
'cargo' 'nextest' 'run' '-p' 'farhelm-ui' '--features' 'desktop' '--lib' '-E' 'test(=tunnels::tests::source_retirement_cannot_replace_a_warning_with_an_unfinished_save)'
```

Run `99697483-ef1a-40d6-8c56-bb8c3102eb3e`:

```sh
'cargo' 'nextest' 'run' '-p' 'farhelm-ui' '--features' 'desktop' '--lib' '-E' 'test(=tunnels::tests::source_retirement_cannot_replace_a_warning_with_an_unfinished_save)'
```

## The review gate's outcome

Every PR passed exactly two independent reviews on gpt-6-astra at high effort: general correctness/design and a separate
observable-behavior review. Corrections went back to those same reviewers. The seven refactoring PRs retain their
behavior-preserving verdicts; the three fix PRs were accepted under the maintainer's explicit drift/focus exception. No
review finding remains open. Temporary model-capacity failures were retried on the required model, without substituting
a reviewer.

Corrections included retaining a stable launcher callback and preserving remembered controls through temporary scroll
entry, returning refused Feedback to Message, and recovering SSH help after a real backdrop click. A proposed radio-
escape scenario was rejected because the actual dialog's footer remains after the radio group; it was not claimed as a
separate observed defect. Commit and PR wording passed fresh-context cold reads. The rebase preserves the reviewed
product diffs, so it reuses those gates.
