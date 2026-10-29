## Since the last review

The same two draft PRs remain. #2807 was rebased without changing its diff. #2808 now removes the automatic-update
preference from the shared fixture that reconstructs a version-26 database, gives the new Settings switch hover text,
and corrects the Update guide and Manage hosts page. The frozen historical database definitions and production migration
are unchanged. These address the latest Decisions following the unsuccessful landing round.

## What this was about

Remote-host updates were easy to miss in the session list. The Hosts button now shows an upgrade arrow and the number of
hosts needing updates. Optional updates get a dismissible card; required updates get a banner explaining why those hosts
cannot be used. Both update exactly the hosts they name or open Hosts.

**Update remote hosts automatically** in Settings under **Hosts** starts off. Enabling it lets the helm perform those
same authorized updates even with no window open. The choice survives helm restarts and is shared across clients.

## Things you should know

The existing Hosts panel's **Update all** retains its broader meaning, including eligible hosts already on the current
build. The reminders and automatic setting act only on outdated hosts. Automatic updates exclude the helm's own machine,
newer builds and busy hosts; busy hosts are reconsidered when their work finishes. The app's own automatic update
setting is separate.

Dismissing the optional card leaves the Hosts arrow visible. The card returns for a newly outdated host, a changed helm
build or an interface restart. Required updates take precedence, cannot be dismissed and hide the optional card. Both
reminders hide while Hosts is open.

A failed or refused automatic attempt waits for that host's next connection or a helm restart. A repeatedly failing host
can therefore be retried on every reconnect. Toggling the setting does not clear failure memory. Planning refusals are
logged; manual Update still shows the reason. Accepted runs keep ordinary progress and failure in Hosts. Turning the
setting off prevents new starts and leaves accepted work alone.

The settings database advances to version 68. This is a one-way upgrade: earlier helms, including the v0.32.0 release
candidates, cannot open it afterwards. No supervisor schema changes. The existing unreleased changelog fragment covers
both PRs, and #2808 removes both source TODO entries.

## Open questions and possible follow-ups

No outstanding product decisions or required follow-ups.

## The PRs

- [#2807 — update reminders in the session list](https://github.com/scode/farhelm/pull/2807/changes), draft, head
  `8f3d818a98c01d628ada04652f9b1f9b5d34c877`.
- [#2808 — automatic remote-host updates](https://github.com/scode/farhelm/pull/2808/changes), draft, head
  `477114bf1234e8e2c4306fd6f1eacb4a819dc88d`.

## Checks run, reused and skipped

Current round, on the combined stack rebased onto main with the four-file correction:

- `cargo nextest run -p farhelm-helm`, recorder `44d635f5-959b-4b2a-8f7f-da895ce653df`: 1,327 passed, none skipped.
  Includes both corrected version-26 cases and the real local and SSH host-update scenarios.
- `bun run build` in website, recorder `6de1fbd6-fdcf-429e-9b6b-6b489270f974`: passed, 33 pages, all internal links
  valid.
- `cargo build -p farhelm -p farhelm-fixtures`, recorder `8b2a65c8-149a-4ab5-977f-15ac825f9716`: passed.
- `dx build --package farhelm-ui --platform web --release`, recorder `6ee94900-d314-4427-88bc-4139aa445d25`: passed.
- `cargo clippy -p farhelm-helm -p farhelm-ui --all-targets -- -D warnings`, recorder
  `8ff964b7-031e-4bd8-a6ab-20e9a2fe6512`: passed.
- `npx playwright test 'settings\.spec\.ts|host-updates\.spec\.ts|tooltip-coverage\.spec\.ts'`, recorder
  `fab39a2b-307e-4cf2-8081-95fa1b40489a`: 94 passed, 47 per engine, none skipped or failed. This runs the full Settings,
  host-updates and tooltip-coverage specs, plus ntfy-settings because it shares the selected filename suffix.
- `cargo fmt --all -- --check`, scoped `dprint check` on both changed docs pages, `git diff --check`, and
  `python -B scripts/check-test-sleeps.py` with the isolated pinned parser environment: passed, 327 delays, none
  unexplained.

Historical evidence remains separate. UI tests at the original draft heads (#2807
`d0402cc2f8a5ef3137d7c5e22e5ed7ad4e58c6a1`, #2808 `7cdc86bd0629c99936fdc5329d52cb4a52ff1adb`) passed: 311
hosts/list/skew cases in `891f8f0e-a28b-44b9-936c-816e92ac3dc6`, and 91 preference/Settings cases in
`e17c2867-2593-4c80-8019-175eef3184f5`. These cover the unchanged classifier, dismissal and preference queue logic; the
rebased combined build and browser checks above cover integration with current main. The earlier changelog format check
remains applicable because the fragment is unchanged.

Earlier failed observations remain retained, not passes: the initial long-card browser attempt
`4b7052b0-e84e-46d6-9766-1630d0391161` (12 passes, two failures); the initial native compile
`144e31b9-541a-4704-b1a4-382f8b1ac406`; affected helm run `2beb4deb-b538-4db6-8fd2-64e26430942c` (434 passes, five
failures); and exact listener/fixture reproductions `8e271c2c-5533-4810-8120-de4b00202ced` and
`d3a336f3-8868-4cd8-9d77-b03fa5cf29a4`. Their corrected focused runs passed in the previous round. Interrupted
`daa2c59c-764d-4bae-877f-a59a72ffd3f2` and `d93a3dc6-0837-4f5b-ac13-61317236ba0d` supply no pass evidence. The later
landing failures, `43959a88-f565-461e-b3dd-760d8725cf4a` (two version-26 fixtures) and
`10af6125-cdc5-4afc-bc9a-369377c00195` (new switch hover text on both engines), prompted this round's corrections.

The current six recorder runs share source head `4936843aa2fbb2ad772b0f8c481d49b294886321` plus the four-file
correction, fingerprint `027b8c9940be69bacca3787b7016811ba95885074e3b0c35751ec7868b0338a8`. All have complete source
identity, complete reports where applicable, stream EOF, no output omissions and normal cleanup. Full retained-output
scans found no runtime skip marker. All three browser stacks cleaned up. Rust uses pinned nextest and tmux with four
global slots and zero retries; browser validation uses three workers, one owned stack each, both engines and zero
retries. No native desktop operation is claimed.

After those checks, the stack was rebased onto main `0f3d4bcdf236d897e34539c7c4c3dc030083fb7b`, preserving the terminal
zoom and new-terminal-button changes that landed meanwhile. The full upstream diff was inspected. The only conflict was
adjacent TODO removals; both sides' removals and main's new entries were kept. #2807's interdiff is empty; #2808 differs
only by that mechanical resolution plus the reviewed correction. Formatting and whitespace checks passed again after
resolution.

The helm tests and backend build remain applicable because no backend inputs changed. UI lint, web build and browser
coverage are reused for the unchanged host-update and Settings code: upstream changes only terminal toolbar visuals,
adds a display-only readout and keeps each control's tooltip attribute; none changes the selected browser specs or their
helpers. Those builds predate the toolbar changes, so they are not claimed as fresh tests of the new toolbar itself. The
website build remains applicable to the two corrected guides; upstream changed a separate page's prose without changing
links, headings or build configuration. The sleep check remains applicable because no delay or changed test premise was
added by the rebase. No additional runtime run was needed for TODO or commit bookkeeping.

Skipped: full workspace and browser batteries, unrelated JavaScript, desktop, installer, provisioning-distribution,
release and live-install checks. The requested helm crate and three browser specs cover this correction; the added docs
passages receive the website build. No preview server was started during this unattended run. No instruction or heading
rename requires the agent-instructions checker.

## Review gate outcome

The revised #2808 passed its required single fresh-context gpt-6.1-sol high review with no findings. The previous exact
Sol/high gate for #2807 and scope checkpoints remain applicable to their unchanged design. The current independent
resume assessment checked the complete upstream Podman and bookkeeping delta and the existing stack; a clean rebase
preserved both PR diffs. A new exact single fresh-context gpt-6.1-sol high review covers the revised #2808. Original
commit and PR wording is preserved from its accepted cold reads. A separate documentation pass covered all four
corrected files. Both PRs remain drafts for the landing monitor.
