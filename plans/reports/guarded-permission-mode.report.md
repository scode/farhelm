## What this was about

Farhelm could show a session as non-YOLO while the agent's own configured default bypassed its safety checks. The
maintainer chose guarded to mean the agent can approve work automatically through a classifier, AI reviewer or sandbox
that can still refuse or ask. Farhelm now selects that mode with explicit launch flags and explains the agent's mode,
flags and limits in the launcher and session shield. It does not promise to ask about every action.

## Things you should know

Cursor takes the agreed fallback: its auto-review flag did not override a configured Run Everything mode. Its non-YOLO
choice is therefore the agent's own setting, shown with a grey dashed shield and treated as YOLO by a host that asks
before those launches. Farhelm does not read vendor configuration or detect later mode switches.

The upgrade adds protocol 48. Update remote hosts after upgrading Farhelm before launching there. Older Claude, Codex,
Grok and Muse sessions launched without permission flags keep their old commands and get the grey own-setting shield.
Ordinary Restart and Replace preserve those commands; Replace asks for confirmation on hosts that ask before YOLO
launches, and agent clone/inherited spawn is refused there. Clone, Replace with and Restart with build a new guarded
command. Restart with can change only the permission to make that upgrade. Older releases cannot read newly stored
guarded launches, so these launches have no downgrade path.

OMP still defaults to YOLO and separately offers guarded write mode and approve. Goose retains its existing choices; Pi
and OpenCode remain YOLO only. The CLI keeps permissions default as an omission alias, including OMP's unchanged
default. Custom command assertions retain their wire meaning and now describe retaining or bypassing safety checks,
rather than equating automatic approval with YOLO.

Real-agent probes used private fixtures and preserved the maintainer's vendor configuration. Claude 2.1.299 and Grok
1.0.52 selected auto over bypass defaults; Codex 0.162.1 selected on-request approval plus workspace-write over
never/full-access defaults; OMP 18.10.4 selected write over YOLO. Fresh and actual saved-conversation resumes were
verified for those four. Muse 1.4.7-R6101.1 selected its auto-review profile over a private persisted unrestricted
default. Muse's Bubblewrap warning means sandbox execution was not exercised. Cursor 2026.10.12-d47ca18 remained Run
Everything under auto-review. These observations cover the tested agents, accounts and versions, not all future
behavior.

## Open questions and possible follow-ups

No maintainer decision remains for this plan. Live detection of a session switching into YOLO remains separate work.
Vendor allow rules, hooks, plugins, always-allow answers and sandbox configuration remain outside Farhelm's launch
guarantee.

Validation exposed an existing remote-uninstall portability problem: uutils stat's missing-file diagnostic has an errno
suffix that the cleanup refuses. The unchanged test failed on Ubuntu 26.04/uutils 0.8.0 and passed in the narrow GNU
coreutils 9.4 run. FLAKES.md records the evidence and TODO.md retains the product follow-up; this plan does not change
uninstall. Three process-group tests failed in the owned build container without an init reaper and passed outside it.
The broader hover-coverage walk also stopped at three pre-existing terminal-link settings radios without hover text; the
targeted permission-help checks establish this plan's behavior separately.

## The PRs

- [#1992 — force guarded agent launches](https://github.com/scode/farhelm/pull/1992/changes), one coherent draft. Pushed
  head `cbb676d46402e0d5d82e07aca109adb0776dffdc`, based on `8f5382d5e36fa45bd793760e3e3a949f29220f4d`.

## Checks run, reused and skipped

The final review corrections passed on Chromium and WebKit: the complete guarded-help/provenance spec and
Claude/OMP/Cursor compact summaries passed 6 of 6 in `5ac8cd57-b1e7-409c-b5ef-f9d0aef5aec4`. The original
compact-summary contract and ordinary tooltip positioning, replacement, keyboard focus, touch suppression and
asset-reinstall cases passed 14 of 14 in `c4c305a8-dad3-48bc-92fa-8a16d45b8a6a`. Accessible descriptions and painted-box
overflow are asserted; desktop and phone screenshots were inspected with the full caveat visible and the documentation
button beside guarded. Earlier explicit template-save checks passed on both engines and remain covered by the unchanged
template correction.

After the first review corrections, the complete UI package passed 463 of 463 in `857b6bb7-9d70-44b2-9e9e-d328c44e39ab`,
and the narrow template cases passed 2 of 2 in `9900a7ec-ecaf-4f94-b204-83d8d13d2946`. Final
`cargo clippy -p farhelm-ui --all-targets -- -D warnings` passed after the search-result, compact-summary and
button-placement corrections. The release web build covering those corrections passed. Rust formatting, dprint,
JavaScript syntax, changelog formatting and the source delay check passed at the final revision; the last source-only
comment clarification reuses runtime evidence because it changes no behavior.

The complete affected proto, helm and UI package run selected 1664 tests, with four nextest slots and no retries: 1659
passed and five failed in run da64f0e2-c88c-4bf0-93d4-19a6749f170e. One stale permission-word expectation was corrected
and passed with the CLI intent case in 063e5f41-6268-4f73-a0b9-1e23ea04be67. The other four passed unchanged in the
narrow outside-container run d37147c8-4273-46eb-92fc-c30942bcdb1e. Three runtime systemd/SSH cases returned SKIPPED
inside the broad run and are not claimed as substrate coverage. The exact CLI invalid-word case passed in
514ed45e-264d-40c3-8c49-71ee44dc7118.

Earlier exact old-command clone/inherit cases passed in 1aad91fd-a694-4543-940a-595a08512c03, and plain Replace
confirmation/copy passed in 8a592e8e-b0b5-47c9-86be-34bf4a98f049. The full protocol package included the protocol 48 pin
and launch serialization tests. Those results are reused after reading the ntfy and desktop-update changes landed on
main: neither changed guarded compilation, stored-command classification or admission. The rebase combined adjacent TODO
removals without product-code conflicts. The corrected UI paths received the fresh checks reported above. The last fetch
added only another plan's blocked queue state, with no product changes; that preserves the pushed change and its
validation.

Chromium and WebKit ran the affected launcher Enter/tabs, remembered permissions, templates and YOLO confirmation specs,
plus hover coverage: 102 of 104 passed in `8b924e97-0760-498d-9e8a-96a4a26a6483`; the two failures were the existing
settings hover gap described above. The focused shield/clone/restart/sidebar batch passed 26 of 28 in
`95af49a0-060e-43fe-97a4-94ad780c71ce`; the two compact recent-row failures were corrected. The permission-only
older-session upgrade and historical OMP comparison passed 6 of 6 in `d9f64be7-0b46-4ee7-9eaa-b1228c034326`. Failed
observations and compile/setup refusals remain retained; retries are separate observations.

Full `cargo clippy --all-targets -- -D warnings` and shipped `cargo clippy -p farhelm --bins -- -D warnings` passed
before the final review corrections. Their unaffected compilation/admission coverage is reused alongside the final UI
lint above. The integrated asset-JS harness passed 241 of 241 in `e536c9b8-503a-4d5c-891a-12cba5b41776`.
`cargo fmt --all -- --check`, `dprint check`, `python -B scripts/check-test-sleeps.py` (298 delays, zero missing
reasons), `node --check` on the tooltip asset and `python3 releasing/check-changelog.py format` passed. The docs
website's frozen install/build produced 32 pages with valid internal links; the subsequent prose clarification and
unrelated updater docs preserve those links and generation contracts, so that evidence is reused.

No full workspace, desktop runtime, installer, release, Mac VM or full browser battery was run: those operations were
not changed. Browser coverage concentrates on permission UI and shared tooltip rendering. The website was not deployed.
No release or live Farhelm installation was changed.

## Review gate outcome

The required fresh-context gpt-6.1-sol high reviewer inspected the coherent change and found three P2 issues: valid
explicit permissions rejected by template saving, clipped mode explanations and missing screen-reader descriptions. All
were accepted and corrected. Its continuation found the same help clipping in search results and an OMP guarded choice
hidden as a default; those were also corrected. The same reviewer's final bounded source verification reports all
findings resolved and no remaining defect. It did not run tests; the checks above are the executor's evidence. The
separate required scope review found no unnecessary subsystem or scope change needed. Commit/PR wording passed two fresh
cold reads after clarifying host confirmation policy and the difference between Replace and Replace with. No review
swarm or implementation delegate was used.

### Landing

Landed alone in its round as #1992. Since the stack was based, main gained only queue bookkeeping, so nothing that
landed interacted with it. confirmation-focus-and-skip was delivered while this landing ran and belongs to the next
round.

#### Release-relevant

The helm–supervisor protocol moves from 47 to 48, so this release requires updating remote hosts: until a host is
updated it sits at "needs update" and launches there wait. Neither database schema version changed; protocol bumps that
added new launch values have not bumped them before either. A launch stored as guarded cannot be read by an older
release. On a downgraded supervisor that is a refusal, not a crash: its session store fails to load, and the sessions
keep running in their terminals but cannot be managed until the host is upgraded again.

#### Review

An independent fresh-context review, focused on the "ask before YOLO launches" guard, found nothing blocking. It traced
every launch path and confirmed the guard is consulted on each one and fails closed. Those paths are:

- user create, fresh checkout, plain Replace, Replace with and Clone;
- Restart with;
- agent-requested create, clone and inherited spawn;
- plain restart, which by design is not re-checked.

Sessions stored before the upgrade without permission flags now count as YOLO there, so protection only got stricter.

#### Fixes made while landing

- The card for approving an agent's launch request showed the permission's full multi-paragraph help in one cell, with
  each line break rendered as a literal escape. It now shows only the mode's heading, such as "Guarded · Claude auto
  mode" or "Not classified · agent's own setting", taken from the stored permission. A new unit test covers both.
- Docs and UI wording still said "runs without approval prompts" for the command tab's YOLO question. The launcher now
  asks "runs in YOLO mode". Fixed:
  - Custom commands, its description and its resume note;
  - the agents overview's command tip;
  - Start a session;
  - Approve what agents do, in two places.

  Start a session's list of launches that ask now also names Cursor launches and a plain Replace of an older session.
  The template editor's YOLO options read "asks for approval" and "runs without prompts (YOLO)"; they now read "keeps
  safety checks" and "YOLO", matching the save panel.
- SPEC.md: "plain Replace asking on guarded hosts" now says hosts that ask before YOLO launches, since guarded now names
  a permission mode. The green shield is described as guarded or an approval mode, since an agent's own setting is
  neither. The agent instructions now say Claude uses auto mode, not auto-review.
- Two browser tests the plan had left stale, which failed on both engines:
  - the approval card's layout fixture asked Claude for the approve mode, which Claude no longer offers; it now asks for
    guarded with Claude's real guarded flags;
  - a sidebar test still expected the label "default" for a recent setup without a permission; it now expects "guarded".
- The approval card is now 401–419 px tall against the layout test's 380 px limit, because the mode name wraps in its
  narrow column. The limit is now 440 px, with a comment saying why.

#### For the maintainer to decide

The reviewer flagged this in the launcher. On Claude, Codex, Grok or Muse, guarded is the default, and the launcher
cannot tell a guarded you clicked from one it chose for you. Switching the launcher to OMP therefore always preselects
OMP's YOLO default, even if you had clicked guarded. On a host that asks before YOLO launches the confirmation still
catches it; on a host that starts YOLO without asking, the launch runs YOLO with YOLO visibly selected.

The landing first changed this to carry guarded over, but an existing browser test ("OMP's default YOLO does not spill
into the next harness") pins the original behavior: switching to an agent gives that agent's own default. So the change
was reverted and the behavior is as built. Remembering an explicit click separately from the default would be a design
change.

#### Other review notes left as they are

- A docs screenshot callout on Start a session still says "default keeps the agent's approval prompts". The next docs
  screenshot refresh should update it.
- Some code comments about "omitted" permissions are stale.
- With an older session, Restart with shows guarded already selected, so upgrading it means clicking a button that
  already looks pressed.
- The FLAKES.md entry this plan added is dated a day ahead in local time.

#### Checks

- Run now, with the review fixes in except the reverted launcher change:
  - `cargo fmt --all -- --check`, `dprint check`, the changelog format lint, and the sleep checker (298 delays, none
    missing a reason);
  - `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`, and
    `cargo check -p farhelm-ui --features desktop`;
  - the supervisor, helm, UI, protocol and CLI unit and binary tests, plus the structured launch, agent relay, wrapper
    launch, restart-with-resume, create idempotency, GitHub checkout and session lifecycle e2e modules, through the
    recorder with pinned tmux, four slots and no retries: 3114 passed, run `81056466-dcec-4be7-8a0b-cb5ca9681ff4`. Its
    224 skips are tests filtered out of the selection;
  - helm, protocol and CLI doctests, run `94f73e31-2a2d-4ba5-9d18-4e76110fbcb9`;
  - the asset JavaScript harness (241 passed) and the website build;
  - browser specs on Chromium and WebKit (approval layout, guarded permissions, YOLO guard, restart with, clone,
    templates, remembered permissions, launcher Enter, launcher tabs, sidebar): 418 passed and 6 failed, run
    `3c64c110-3138-41fc-ac1e-e8dea13a8958`, kept. The six were the three tests described above, each failing on both
    engines.
- Run after the revert and the fixture fixes:
  - formatting, `dprint check` and full all-target Clippy;
  - protocol, UI and CLI unit and binary tests: 776 passed, run `6a354376-917e-4ab0-9496-bd5d6c38b4ce`;
  - the approval layout, remembered permissions and sidebar specs in full on both engines: 250 passed and 2 failed, run
    `7e825b40-dad0-40db-8f71-03aa867395fd`, kept. Both failures were the approval card height on the two engines;
  - after raising the height limit, the approval layout spec on both engines: 20 passed, run
    `9fc05335-3b5d-4be4-9321-a9e05fcd8be3`.
- Skipped: the full workspace and full browser battery, desktop runtime, installer, provisioning and release gates. The
  change does not touch those paths, and the selections above cover every permission surface it changes.
- Still unverified, as the report says: real agents beyond the versions the executor probed.
