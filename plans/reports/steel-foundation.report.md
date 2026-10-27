## Since the last review

None of the five draft PRs has merged. This round changes the session-list movement test setup, a long-path alignment
test and its flake record in #2732; #2728 through #2731 are rebased without source changes. No PR was added or dropped.

The reported intermittent failure was a font-readiness assertion, not an overflowing sidebar. In all seven retained
failures from the two landing runs and two focused repetition runs, the sidebar-fit assertion passed before the global
font-loaded assertion failed. The test parks the pointer over a session row to hold the list order. That starts its
delayed hover card, whose bold title first requests Inter Bold after the test's earlier font wait has finished. A
retained failure trace confirms that ordering. The floating card does not contribute to sidebar layout; no sidebar
height regression was established.

The test now loads the declared fonts before pointer-dependent geometry. Its page dimensions, pointer position, font
checks, strict geometry comparisons and menu assertions are unchanged. This is a test starting-condition correction;
product code is unchanged. The bounded Opus consult independently confirmed the cause and recommended this correction.

The required whole-file repetitions passed: all nine tests ran 20 times on each engine, 360 passes, zero retries or
skips. The full browser run then finished with 2681 passes, one failure and 14 skips; the corrected movement tests
passed on both engines. The remaining failure was an older long-path alignment assertion: WebKit reported exactly one
CSS pixel against a strict `< 1` check, despite waiting for fonts. The test now accepts `<= 1`, matching neighboring
row-alignment checks, and includes its box and scroll values in failure messages. Its independent overflow, front-edge,
logical-text, direction and ellipsis assertions remain. That exact test passed once on each engine afterward. The full
command remains failed; it was not rerun after this test-only correction.

## What this was about

This is the shared foundation for Steel: blue-gray dark colors, a light counterpart, solid periwinkle selections, Inter
for interface text, and shared button, switch, menu and dialog styling. Paths, commands, versions, ages and terminal
text keep JetBrains Mono. Settings gains a per-device System/Dark/Light appearance choice. The terminal keeps its
existing dark theme. The later Steel plans own the host rail, session toolbar and tunnels, later dialog layouts, and
sentence case. The reference mockups and the shared TODO entry remain unchanged.

## Things you should know

The preceding follow-up restored the terminal's #282c34 background by correcting stylesheet priority. It renamed the
launcher permission-choice group to permission mode, while keeping the visible label and Permissions search section.
Selected local and remote host marks use the same inverse selection color, as requested. The effort dropdown is included
in the specification's existing Enter-to-launch contract. Those product and specification corrections remain.

The other earlier landing failures were test corrections for intended behavior or test setup. Dialog tests now follow
Cancel-first ordering, with the long-title confirmation retaining containment checks. Launcher tests use the native
effort dropdown, establish its focus before reopening the model picker, and compare the padded permission segment's
center with its help control. The template typography test treats shortcut digits as technical values in JetBrains Mono
while requiring prose in Inter. The drag-copy notice test uses the same integer mouse-release coordinates for dispatch
and measurement; the title-band test measures the visible palette wordmark; the terminal link-click test clicks the
native radio's visible label and checks its actual state and terminal behavior. Host rediscovery admits the existing
idle-with-new-output status. The template editor fixture awaits initial Cancel focus before testing shortcut focus
preservation. The two-host planning fixture excludes unrelated fake-host retry updates while keeping both plans held
behind Settings. The earlier movement-font wait was insufficient and is superseded by this round's preload. Source
maintenance formatted the interface's Cargo.toml; specification maintenance records the requested selected-host mark and
effort Enter contract. These were not additional product changes beyond the restored terminal background and requested
accessible group rename.

The sidebar investigation found an inaccurate premise in the latest Blocked text. Runs
9a3c0404-de1e-4a81-8af2-dc4bad149e19, d00d80ec-381b-4ba3-9684-1619781fa98e, b31c6a07-16b9-4b74-8bfd-bb7f4e4e8ecf and
85ec7662-61f7-4e76-b486-d54cdb2ced8d all failed this file's global font-loaded check after passing its fit check. In one
d00 failure, the font wait resolved before the hover card requested Inter Bold; the later failure diagnostic already
showed fonts loaded again. The other failures match the assertion and shared pointer setup; their individual network
timelines were not all inspected.

The card sits in a fixed body-level tooltip, so this delayed load cannot make the sidebar taller. Its first title can
briefly use a fallback font under the existing font-display: swap policy. That is ordinary first-use font loading, not
an established row or sidebar layout defect. The fix establishes the settled-font premise required by these movement
tests without changing application loading policy.

The long-path recurrence shows the previous font wait did not cure that separate test. WebKit scroll-origin or
fractional-layout rounding is a hypothesis, not an established cause. The retained failure screenshot was taken after
cleanup and does not show the measured path, so it cannot establish whether that path looked correct. The new flake
entry preserves that limitation and the exact change from an exclusive to an inclusive one-pixel endpoint. Product CSS
is unchanged.

## Open questions and possible follow-ups

No maintainer decision is pending for this correction. The visual evidence remains representative ordinary appshots; it
makes no transient-paint, native macOS or physical-input claim. Active host operations, pending Trash, loaded SSH help,
the installed updater and successful uninstall planning retain the preceding code-only visual coverage.

An earlier artificial native sequence rapidly changed appearance and immediately reopened Settings. It briefly showed a
Light card over a Dark page and stale section content. Ordinary input has not reproduced it; neither a cause nor a
product fix is established. No further native timing diagnostics were added. Existing A+ clipping at a 650px main pane
belongs to the later session-toolbar plan; mixed sentence case belongs to the final wording plan. No live installation
was changed.

## The PRs

- [#2728: unpublished visual-review appshots](https://github.com/scode/farhelm/pull/2728/changes).
- [#2729: Inter interface typography](https://github.com/scode/farhelm/pull/2729/changes).
- [#2730: Steel dark palette and solid selection](https://github.com/scode/farhelm/pull/2730/changes).
- [#2731: device appearance choice and Steel Light](https://github.com/scode/farhelm/pull/2731/changes).
- [#2732: shared controls, menus and dialogs](https://github.com/scode/farhelm/pull/2732/changes).

Final pushed heads, respectively: 8b97bf57a652, efc0971b8e2c, 027578c576dc, da0f86374ea8 and 694621c0c8eb. All five
remain drafts in one linear stack; the executor marked none ready and merged none.

## Checks run, reused and skipped

Current source-only `python -B scripts/check-test-sleeps.py` passed using the prepared isolated interpreter: 322
inspected, zero missing reasons. `dprint check FLAKES.md` passed. The repository's dprint plugins do not format this
TypeScript file, so its no-files result is not claimed as a formatting pass. Playwright executes the changed TypeScript;
there is no separate e2e typecheck command in the package's scripts or validation inventory.

The initial whole-file run, 8955c750-e7a4-4d0a-8270-49e7845d7736, hit the recorder's three-minute deadline: 17 passed
and the last WebKit case was interrupted. It remains a timed-out command, with complete output and retained artifacts.
Its short deadline was not reused for repetition.

Run 84ed17c8-f465-4dae-a62c-2fa85a120f9d passed the whole movement file with
`npx playwright test tests/popover-identity-movement.spec.ts --repeat-each=20 --reporter=line,json` from `e2e/`, on both
engines: every one of the nine cases passed 20 times on Chromium and 20 times on WebKit, 360 passes total, zero skips,
failures, retries or global errors. It took 32.4 minutes with three workers and pinned tmux 3.7c. This used generic
command recording because strict browser mode does not allow internal repetition. Counts and resolved policy were
checked directly in its retained JSON; they are not recorder-validated browser counts. Source was
30fe006abd3bb9ef3debb24fbd9dd385108679b4 plus the test delta, with complete fingerprint
bb5555bf958b1f7f45705754db3ed438e9b273bf76a9dccf20216542dcdde39a. All 101824 output bytes were retained, EOF was
observed, cleanup completed without force, and the output scan found no runtime SKIPPED marker. Folding into the
original PR preserved the tested tree byte for byte.

Full browser run f6bc51f5-3985-441d-b5ce-17e4bfd8e648 (`npx playwright test`, three workers, both engines, no retries)
finished in 1.8 hours: Chromium 1346 passed and two skipped; WebKit 1335 passed, one failed and 12 skipped. There were
no global errors or flaky outcomes. Clean tested source was cef8e2a12524ffd5d9fd5df6153e727dbb5a50fb, complete
fingerprint 4f843a4e98508ec1c09b48c2ad1988e41dcf95c016873df3f209e197634d858e. Its raw JSON exceeds the recorder's 16 MiB
inspection cap, so these counts were checked manually against the raw report and complete console, rather than presented
as recorder-validated. All 1284466 output bytes were retained, EOF was observed and cleanup completed without force.
Both child and recorder exited 1; the whole failed UUID is retained.

The 14 skips are four deliberate real-vendor-agent cases, nine WebKit clipboard cases that Playwright cannot grant the
required permissions, and the WebKit copy of a Chromium-only visual matrix. Their retained skip explanations were read;
those substrates are not claimed as covered.

After the inclusive-boundary correction, b03e96d6-56c9-4afc-9fc8-f51d343bed70 ran
`npx playwright test tests/sidebar.spec.ts -g 'a long cwd shows its tail with characters in logical order'` from `e2e/`,
once in Chromium and once in WebKit: two passes, zero skips or retries, 12.3 seconds. Its strict browser report is
complete. Source was cef8e2a12524ffd5d9fd5df6153e727dbb5a50fb plus only that test correction, complete fingerprint
27bdf8253048f41826b806d07988782114ed6fd8328137112564c24819149ec7. All 7167 output bytes were retained and read, EOF was
observed and cleanup completed without force. This pass does not replace the failed full run or prove the engine
rounding hypothesis. No second full suite was warranted for a local assertion endpoint and diagnostic-only change.

The application build from e2a41109-01cd-4cf2-803c-71840625b13f is reused. It passed against source
121fca4d450c0c78f8a2e2bf6d21d02868c44d1c with complete fingerprint
0159460f8aadeb4b8a755cce4f521fc4e40248869dea5874d0a17b4fd5299032. Subsequent changes affected tests, specification text,
the flake ledger and queue records; the built product is unchanged and its artifacts remain present. No rebuild was
needed for this test-only correction.

The preceding full run c909ec87-c8e1-4ae1-b0bb-e849195af2c9 remains failed: 2679 passed, three failed, 14 skipped. All
36 original landing failures passed there. Three older fixture-readiness corrections subsequently passed in focused
both-engine runs 92271d2f-6ba5-47e5-a622-b9a30684d6f8 (8/8) and 2fa75d14-9eb8-4c63-a070-0ac26c415d70 (4/4). These are
historical observations, not this round's full-suite result. The old full report exceeded the recorder's 16 MiB
inspection cap; its counts were checked directly against the complete console and raw JSON, not presented as
recorder-validated.

The earlier CSS checks passed 53/53 in 4e4e2f86-76a6-4f50-85e7-b5f4ae5c59c0 at the same build source and fingerprint
above; their CSS is unchanged. Earlier Rust evidence keeps its actual baseline scope: UI library
85b323ce-5cd5-4442-a7c5-af3377b0dded passed 783 tests at the Inter step, and typography
572f1230-80d1-4622-90bf-3164de9a84d1 passed 29 selected tests after font corrections. Asset JavaScript
3570be6e-81dd-4a26-8da5-1fdbf4f18eaf passed 358 tests. Native appearance c772ca7a-6aec-4dff-89ac-9bbeb2e024fe retains
startup, persistence, font, embedded-asset and same-terminal input/output evidence. None is described as a fresh
final-tip battery.

| Reused run prefix | Recorded HEAD / complete source fingerprint | Selection                                                      |
| ----------------- | ------------------------------------------- | -------------------------------------------------------------- |
| 85b323ce          | 6d83cb62cc2e / 0d9960f1c5e3                 | cargo nextest run -p farhelm-ui --lib                          |
| 572f1230          | 6d83cb62cc2e / 2dde68f8fe78                 | Same UI command, typography/template/destination-detail filter |
| 3570be6e          | 4a9f5fb5ee2f / 13258aefa2da                 | node --test in the UI asset harness                            |
| c772ca7a          | 19e9eadd86ae / ccc8ad5d96b0                 | DESKTOP_SMOKE_APPEARANCE=1 scripts/desktop-smoke.sh            |

The identities above are prefixes of the complete retained records. They retain their preceding baseline scope because
this round changes only browser tests and the flake ledger, with no Rust, asset JavaScript, native code or product
styling changes.

Ordinary appshot run a0c822bd-4cc1-4777-8bc8-3fdf7ecd32a3 retained valid images before its selector failure; eight
main-window/terminal-settings images supplied the preceding representative visual review. After that selector was
corrected, e4acb3b3-e5f9-4c34-a950-3dbe50ddf5c2 passed staging and the 46-surface dark-wide capture. The failed matrix
remains failed. Product pixels are unchanged in this round, so those representative captures and their stated limits
still apply. No additional appshots, Rust battery, native smoke, installer or deployment checks were needed for the
test-only corrections.

## Review gate's outcome

Fresh gpt-6.1-sol/high review 0822afca-3eef-44ec-865e-200103e717a7 found no issue in this test-only delta and confirmed
all geometry, font, menu, focus and hold assertions remain. Parent inspection agreed. Bounded Opus 5.5/high consult
4c1fe407-76b7-4382-9443-a8235d3372cc independently identified the delayed hover-card font trigger; that consult is
separate from the code gate and from visual review.

Final gpt-6.1-sol/high review 127ed937-e73e-4cc3-bb9f-a15920f96fae found no issues in the alignment-test and
flake-record delta. Parent inspection agreed that the inclusive endpoint preserves the distinction between clipping the
path's front and clipping its tail. The reviewed delta was folded into the original PR without changing the tested tree.
The bounded Opus 5.5/high follow-up consult 3c1b469a-58df-4d36-94b6-5b4828a5e333 recommended matching the neighboring
inclusive endpoint; its engine-cause hypothesis remains unproven.

The previous required code reviews of the five original units and the preceding corrections remain applicable to
unchanged source. The preceding Opus 5.5/high visual gate inspected eight ordinary images across dark/light and
wide/650px main-pane layouts, with host-colored-frame details judged from code and browser behavior. This round changes
no visible product behavior and requires no new visual gate.
