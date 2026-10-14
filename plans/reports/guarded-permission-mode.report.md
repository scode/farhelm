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
