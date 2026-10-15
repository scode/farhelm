### What this was about

In the session launcher, choosing Guarded and then switching to OMP could select YOLO even though OMP supports Guarded.
The maintainer decided that every agent offering Guarded should default to it, and that any automatic switch to a more
dangerous permission mode should be hard to miss.

### Things you should know

New OMP launches now default to Guarded. Existing stored launches keep their recorded meaning, including historical OMP
launches with an omitted permission that meant YOLO. Remembered choices and when defaults apply are unchanged. The other
agents offering Guarded already defaulted to it. The latest public release was checked: v0.28.2 uses protocol 47, before
Guarded was introduced in protocol 48, so this does not reinterpret a released Guarded OMP launch.

An agent switch that automatically raises the permission risk now shows a red banner naming both modes and explaining
that the destination agent does not support the previous mode. It offers Undo and dismiss, announces itself to screen
readers, and highlights the permission control. Its roughly eight-second lifetime pauses for hover or focus; reduced
motion keeps a static outline. Direct permission choices and choosing the first agent do not show it. Search, templates
and cross-agent model choices use the same completed-action comparison; a template's explicit permission is deliberate.

Undo restores the complete pre-switch draft, replacing edits made after that switch. The plan assumed that snapshot
already existed; it did not. A scope review checked the alternative, and the implementation keeps one bounded snapshot
of editable values, using existing destination and submission invalidation. A newer Clone or Replace retires that Undo,
including an already queued click. Ambiguous managed-checkout retries retain their existing reconciliation behavior.

Farhelm still does not recognize OMP approval prompts. A Guarded OMP session waiting for your approval shows as running
or idle and raises no waiting alert. The specs, affected documentation, changelog and retained decision are updated; the
original finding is removed. The website has not been deployed, and its Overview page and artwork were not edited.

### Open questions and possible follow-ups

None required. Undo deliberately restores the whole pre-switch draft, so later edits in that same launcher are replaced.

### The PR

- [#2055](https://github.com/scode/farhelm/pull/2055/changes): Guarded defaults and conspicuous permission escalation
  recovery; one draft PR based on main. This executor has neither marked it ready nor merged it.

### Checks run, reused and skipped

- The focused recorded Rust selection passed 10/10: Guarded defaults, preserved stored interpretation, agent switching,
  actual fresh/resume flags and permission-risk grouping. Run `2425b262-0c2a-47bf-94ae-5b4d6b2ca138`.
- An initial default selection passed 102/105 and exposed three stale expectations introduced by this change: old OMP
  compiler flags, an omitted-YOLO recent fixture using OMP, and existing lowercase message wording. The exact corrected
  cases passed 3/3 in `004400e7-d7fd-4ccc-9657-9e00ed5fbcdf`. The failed run `eda5f6d6-eead-4014-8c0f-0b57a33bb214`
  remains retained; it is not counted as a pass or logged as a latent flake.
- Browser run `1b2d9056-61ba-441c-a09b-4f84510a594e` passed all ten new warning/default/Undo/timer executions and both
  existing cross-agent model cases across Chromium and WebKit. Its two compact-tooltip failures came from an incorrect
  test expectation: visible rows say “defaults”, but tooltips retain “permissions: guarded”. That failed run is
  retained.
- Narrow browser run `ad2f7a7c-7ab8-4676-a71e-cbdfcab642c9` passed 6/6 across both engines: corrected compact tooltip,
  mounted Clone reseeding with a discriminating stale-Undo probe, and the default/full-draft Undo case with saved
  screenshots. Both screenshots were visually inspected. An earlier mounted test passed 2/2 in
  `fdf860bf-f826-4ba3-b11e-bc2bad1b9695`; its later stronger version is covered by the six-case run. All browser runs
  used one worker, zero retries and pinned tmux, with no skipped cases and completed owned-stack cleanup.
- CLI and release web builds, workspace all-target Clippy, and the frozen-install website build passed. The opening
  generation correction then received a new web build and UI all-target Clippy pass. Formatter, applicable changed-file
  dprint, whitespace, changelog format and isolated delay checks passed; 299 delays had zero missing rationales.

The careful rebase onto `111808d7` incorporated queue/report bookkeeping and the older-template editor fixes in
#2051/#2052. Their full incoming diff was inspected: separate editor behavior and spec paragraphs, with the shared
template shape and launcher replay contracts unchanged. This change's code, CSS, tests and documentation pages were
byte-identical across the rebase. Runtime results and source review were reused; spec/ledger/index formatting,
whitespace and changelog checks ran again. The final amendment only adds the real PR link.

The full Rust and browser suites, native desktop runtime, installer and release gates were skipped: focused tests cover
the changed default, compiler and launcher paths, and no remaining interaction required broader execution. The pure
JavaScript harness does not exercise this Rust/CSS behavior. Browser evidence covers real local helm/supervisor
fixtures, not live vendor agents or the native desktop shell. The watchdog remained active throughout execution.

### Review gate outcome

The required fresh-context external gpt-6.1-sol high source review found one lifecycle defect: Undo was bound to its
notice but not to the launcher opening. That was fixed with the existing opening generation. Follow-up caught the
tooltip expectation and strengthened the mounted-generation test so a later reseed could not hide an unauthorized
restore. Final review reported no actionable findings and no unnecessary machinery. It received the full plan, original
decision and verbatim test-authoring checklist. A separate cold read accepted the commit/PR wording; a documentation
pass covered every touched file. Static review and runtime evidence are distinct above.

Implementation and investigation stayed in the main session, as required. Private galaxy-brain session
`e81ebbb6-031d-4d50-93f6-ff1f857d5edb` retains review decisions, outcomes and raw external counters. Some events were
recorded retrospectively; exact served-model attribution and native-reader/main-session usage are unavailable. No token
totals or cost claims were inferred. The report receives its own native cold read before delivery.

### Landing

Landed as #2055, the round's only plan. Since its base, main had gained the two Settings fixes (#2054, #2053), which
touch neither the launcher nor the permission defaults; the rebase was clean.

#### Review

An independent fresh-context review confirmed the security core. Every agent that offers Guarded now defaults to it for
new launches, enforced by a test, while Pi, OpenCode and Goose stay YOLO and Cursor keeps its own setting. Stored
launches keep their recorded meaning: an OMP launch stored without a permission still means YOLO for Restart, the row's
shield, the YOLO confirmation and approval cards. Recents, templates and Clone prefill now read an omitted OMP
permission as Guarded, which only ever makes a launch safer. There is no wire change: the helm fills in omitted
permissions and supervisors receive only the compiled command, so mixing versions cannot turn a new omission into YOLO;
the latest public release is protocol 47, before Guarded existed. The banner covers agent clicks, search, templates
without a permission and cross-agent model picks, and never deliberate choices; its Undo is fenced by the notice, the
launcher opening and the busy lock, and cannot revive a submitted launch. The change adds no per-harness branches in
shared code.

#### Fixes made while landing

- Two existing browser tests still expected OMP to default to YOLO and would have failed: the remembered-permissions
  check that YOLO defaults do not spill into the next agent (OMP removed from its list, as the PR had done for the
  matching sidebar loop) and the OMP composer test (now expects Guarded; its comments distinguish stored launches, which
  keep YOLO, from new ones).
- The JS harness, which the executor did not run, lints the stylesheet and refused the banner's alternating highlight
  because it set only `animation-name`; it now uses the full `animation` shorthand, and reduced motion still turns it
  off.
- `farhelm`'s `--permissions` help still said OMP defaults to YOLO; it now says agents that offer guarded default to it.
  Three code comments said the same and were corrected.
- The docs agent table's OMP tooltip said "YOLO by default"; the launcher page now mentions the red banner and its Undo;
  "unless you chose or remembered another mode" now says the launcher remembers.
- At phone width the launcher's padding is 16px, not 24px, so the banner would have stuck out sideways and been clipped
  at the top; it now uses the narrower offsets there.
- Clicking a recent replaces the whole draft but left an earlier banner in place, describing a switch that no longer
  applied, with an Undo that would throw the recent away; a recent now clears it.
- The reduced-motion test only checked the outline width, which holds either way; it now also checks that the highlight
  animation is off. The stylesheet comment claiming nothing else sets `animation-play-state` was corrected, and two
  sidebar test docstring and title leftovers were fixed.

#### Left as is

- Four launcher browser specs fail on both engines and appear to predate this plan: `composer-word-search`,
  `cursor-composer`, `goose-pi-composer` and `grok-composer` count the permission picker's buttons or search for a
  "Permissions: default" option, and the counts match the guarded and custom-flag buttons added by #1992 and #1995 and
  the rename of "default" to "guarded" there. This plan does not change the picker. They were not run on main to
  confirm. Fixing them is a test-only follow-up.
- A template that resets the permission to default counts as a deliberate choice and shows no banner; only a template
  that leaves the field out counts as automatic. A `harness:` search accepted from the Command tab changes the hidden
  agent draft without a banner, because the mode was not on screen. Both defensible.
- Nits: a test name still says "default-YOLO" for templates, and one Rust test lists the Guarded harnesses by hand.
- Suspected, not seen in this run: a sticky banner could cover top controls in other specs that switch from a Guarded
  agent to Pi, OpenCode, Goose or Cursor.

#### Checks

- Run now, on the rebased change with all fixes in:
  - `cargo fmt --all -- --check`, `dprint check`, the changelog lint and the sleep checker (299 delays, none missing a
    reason);
  - `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`, the UI crate's clippy
    with the desktop feature and `cargo check -p farhelm-desktop`;
  - the protocol, helm and supervisor packages in full through the recorder with pinned tmux: 2361 passed, 2 skipped;
  - the UI crate in full with the desktop feature, run `e96fdaaf-2f04-41b8-91e1-46a827b8735b`: 635 passed;
  - the JS harness (248 passed) and the website build;
  - twenty launcher, permission, template, clone, replace, sidebar and tooltip-coverage specs on Chromium and WebKit,
    run `9fb35708-4387-4a36-8612-b6aef47c21a6`: 552 passed, 8 failed, the four pre-existing specs above on each engine.
- An earlier run with the original change stopped at the JS harness failure described above.
- Not run: the rest of the Rust workspace and other browser specs.
