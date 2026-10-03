## What this was about

The sidebar's open padlock for YOLO could look like a promise that the session was locked down. Other rows often had no
permission mark, and the sidebar and launch confirmation used different rules to recognize YOLO. The launcher also
offered an unmarked default for OpenCode, OMP and Goose even though their stock modes act without approval.

The maintainer chose three marks: an amber slashed shield for YOLO, a green shield for a non-YOLO launch made through
Farhelm’s normal launch dialog, and an amber question mark when a profile or custom command does not establish the
permission mode. The stack implements those choices, makes the sidebar share the confirmation's classifier, and aligns
the internal host-setting names with the wording users see.

## Things you should know

OpenCode, OMP and Goose now start with YOLO selected, like Pi. OMP and Goose still offer their explicit approval modes.
Farhelm passes the YOLO option explicitly, overriding vendor configuration that asks for approval; OpenCode also stops
asking about paths outside the project. These launches require confirmation on a host configured to ask, including clone
or replacement of an older session whose saved permission was omitted.

A default YOLO launch clears remembered permission preference, so the next Codex or Claude launch does not inherit it.
Explicit YOLO on a harness with a non-YOLO default remains remembered. Switching through a default-YOLO harness discards
any earlier explicit YOLO choice when switching back: the destination returns to its default permission mode. The draft
does not track where that choice originally came from. A preference saved before upgrading retains its value until the
next structured launch, because the old stored preference has no harness provenance.

Raw commands never receive the green shield. Unrecognized commands, plain shells and `codex --full-auto` get the
question mark. A conservatively recognized YOLO command gets the slashed shield: ambiguous `env` prefixes can therefore
receive that mark even when the command might not actually bypass approval, matching the existing confirmation policy.
The tooltips and screen-reader text retain the specific approval mode.

This stack changes the helm/supervisor protocol to version 36 and renames a database column in schema 34. Update hosts
with the helm; this release cannot be downgraded in place. The hidden `--allow-yolo-on-sensitive-host` CLI alias remains
accepted. REST field and route names change with the bundled UI, without aliases.

The README screenshot was not refreshed, as the plan explicitly excluded a new capture. Its staging script uses raw
commands, so its ordinary rows will show question marks on the maintainer's next capture; using green shields there
would require structured launches.

## Open questions and possible follow-ups

No decision is needed to review this stack. The upgrade-time preference caveat and the README staging behavior above are
deliberate limits, not claims that an existing preference or screenshot was migrated.

## The PRs

- [#1509](https://github.com/scode/farhelm/pull/1509/changes): align host confirmation names, including protocol and
  database migration.
- [#1510](https://github.com/scode/farhelm/pull/1510/changes): use one YOLO classifier for confirmation and session
  rows.
- [#1519](https://github.com/scode/farhelm/pull/1519/changes): make default YOLO explicit and keep it from carrying into
  another harness's default.
- [#1520](https://github.com/scode/farhelm/pull/1520/changes): show the three permission marks on every row. All four
  PRs remain drafts.

## Checks run, reused and skipped

- Ran the default-launch, remembered-permission and YOLO-confirmation browser specs on Chromium and WebKit: 44 passed,
  recorder `ffa89bf0-9327-4103-a88d-6059097d8ede`. These cover actual successful default launches and both browser and
  helm memory, explicit approval choices, and confirmation/host-setting requests.
- Ran 20 further browser cases on both engines: all passed, recorder `6258361c-3ed3-4f6f-acf5-6ce0ff0fea2d`. These cover
  model-picker harness switching, old omitted-permission restart comparison, actual clone flow, restart/replace consent,
  exact permission colors and descriptions, and narrow-row alignment. Inspected both engines' rendered captures: all
  three marks remain distinct in the fixed sidebar slot.
- Ran 74 focused Rust tests covering permission defaults, composer reconciliation, confirmation wording, and structured
  launch inheritance and process argv: all passed, recorder `fac5b4e1-7d92-403d-8e59-1ab2a426d8c3`. The process tests
  ran against pinned tmux; no runtime substrate skip was reported. The 21 row mapping tests also passed, recorder
  `0c0ede81-064b-4a3b-990e-d97832789cc0`.
- Reused the rename's 29 migration, REST and guard tests at `cc831030`, recorder `13c92eb7-7f33-4811-a602-3f1d669e5382`,
  and the strengthened schema-34 migration test at `32e2aae0`, recorder `b54e9658-12ec-409a-9bf5-d83068efb7e9`. Later
  changes do not alter the renamed transport fields or migration. Reused the 25 classifier/row tests at `9161065e`,
  recorder `a0f436a7-339c-47c1-a2ef-b1b3fe6cdea3`; the later row and browser checks cover the new rendering.
- The broader default-permission run at `7c09bc27` passed 107 of 108 tests, recorder
  `fe64cedf-f71c-4c74-af5c-78c6837e8bd8`. Its failure was an outdated OpenCode test expectation after default
  normalization, not a latent flake. The unchanged omitted input now expects explicit YOLO and `--auto`; that case
  passed its narrow rerun, recorder `22ecd98d-2835-4c99-9074-1652149cd798`. The other successful compiler, store and
  memory tests remain applicable; subsequent production changes were confined to browser reconciliation and display.
- Ran both Clippy configurations, `cargo fmt --all -- --check`, targeted `dprint check`, changelog validation, and the
  isolated test-delay checker (274 delays, zero without rationale): passed. Built the CLI and web UI for the browser
  tests. `cargo check -p farhelm-ui --features desktop` passed.
- Rebased across five bookkeeping-only main commits: two plan reports, two claims, and the update-while-running
  plan/TODO addition. Their diffs changed no runtime behavior or specification contract used here; the new TODO entry is
  preserved. Reused the checks after this rebase.
- Skipped full Rust/browser batteries, desktop runtime, installer, provisioning and release checks: the focused tests
  cover this stack's changed permission and transport behavior. The desktop renderer compile checks the renamed desktop
  caller; this stack changes no desktop bootstrap, installation or release machinery. The README capture was explicitly
  excluded by the plan.

## Review gate's outcome

Each of the four review units received its required fresh-context Opus 5.5 review at high effort. Findings led to a
stronger migration check, removal of a conflicting blue CSS rule, corrected compiler and browser expectations, repair of
model-picker default spill, visible remembered YOLO before harness selection, normalized old-session restart
comparisons, and stronger accessible-description assertions. The default-launch follow-up confirmed the production fixes
and identified two new browser-fixture mistakes, which were corrected before execution.

The default-memory rule remains the plan's value-based rule rather than adding provenance state. The display keeps using
the shared effective-permission rule, including its fallback for unsupported older values; the helm rejects unsupported
explicit values at admission. A speculative duplicate YOLO fallback was declined because it would reintroduce a separate
display classifier. No unresolved demonstrated correctness finding remains.

Commit and PR wording passed fresh cold reads. The report passed a separate fresh-context cold read before delivery.
