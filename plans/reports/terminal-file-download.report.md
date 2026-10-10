## What this was about

Agents often return a file by printing its path, including a path on another host. Farhelm previously offered no way to
save that file from the terminal. The maintainer chose a hover check that discloses the source, followed by an explicit
click, with a 100 MB limit and native Downloads saves in the desktop app.

The stack recognizes absolute and home paths, relative paths containing a slash or file extension, and OSC 8 file links.
A fresh hover shows checking immediately, then the session host, resolved path and size, or the reason it cannot be
downloaded. Browser clicks save the complete file under its basename. Desktop clicks save directly into Downloads, pick
an unused name, and show the saved location. The covered TODO entry is removed.

## Things you should know

Relative paths use the session's launch directory. Farhelm does not follow later shell directory changes. Ordinary
printed paths stop at whitespace, quotes and brackets; paths containing spaces can be supplied through an OSC 8 file
link. Diagnostic line/column suffixes and sentence punctuation are excluded. Plain words, numeric versions and
scheme-bearing plain text are not file links. Existing http(s) links retain their behavior; OSC 8 accepts only http,
https and file, so enabling file links does not admit script or data links.

The source text is controlled by the agent. Hover discloses the resolved host and path before a click is admitted, and
nothing downloads without that click. Any regular file readable by the session's Unix account is within reach, including
symlink targets; this uses Farhelm's existing host trust model. Agent-authenticated peers cannot request these reads.
Directories, unreadable or missing files and files over 100,000,000 bytes are refused. The size bound is enforced again
while streaming, including growth after admission. Failed transfers show an in-page message; unfinished native saves
leave no partial download. Browser saves buffer the complete file, which is why the size limit is deliberately modest.

The host protocol moves from version 43 to 44. A mismatched supervisor must be updated before connecting, through the
existing update path; no new compatibility layer or installer behavior was introduced.

I inspected real Chromium screenshots and saved bytes from local and SSH sessions after widening the sidebar by dragging
its edge: both hovers showed the owning host, complete path and size, and both saves matched the source bytes.

I inspected a real Linux desktop window: hover disclosed the host, full path and 27-byte size, a click saved the
expected bytes to an isolated Downloads folder and showed its location, and a second click saved an identical copy with
an unused name. macOS was not tried.

## Open questions and possible follow-ups

No implementation decision is awaiting the maintainer. A macOS native-window check remains useful because its folder
resolution and embedded engine were not exercised here.

## PRs

- [#1769](https://github.com/scode/farhelm/pull/1769/changes): host-side file inspection and bounded streamed reads.
- [#1771](https://github.com/scode/farhelm/pull/1771/changes): authenticated helm endpoints and native Downloads saves.
- [#1773](https://github.com/scode/farhelm/pull/1773/changes): terminal recognition, hover, click, specs and user docs.

## Checks run, reused and skipped

Implementation and runtime validation used an owned Linux container with four CPUs and 12 GB memory, pinned nextest
0.9.143 and tmux 3.7c. Rust selections used four slots and zero retries; browser selections used one worker and zero
retries. No selected successful run printed a runtime substrate skip. The SSH browser fixture's connected state was
explicitly established.

The pre-rebase checks cover the behavior pushed as `325a8b1fbdc8`, `da88f155c0c1` and `3e15bc10194e`, including recorded
dirty-tree snapshots before those commits. Comparing the entire stack diff before and after rebase preserved every added
and removed line. Checks reused below still cover those same contracts.

Runtime commands below ran through `python3 scripts/record-test-run.py`, with `--runner nextest` or
`--runner playwright` for the corresponding suites and generic recording for JS/manual attempts. Rust and browser runs
used `--tmux required`; pure JS used `--tmux none`. The focused child commands were
`cargo nextest run -p farhelm-supervisor -p farhelm-helm -p farhelm-proto --lib -E '<download units, restricted peer and version pin>'`,
`cargo nextest run -p farhelm --test e2e -E 'test(file_downloads::)'`, and
`cargo nextest run -p farhelm-helm --lib -E 'test(downloads::tests::)'`.

Before rebase, recorded supervisor/protocol units passed five cases in `e4021572-a966-4308-bbee-942312da9bf1`;
file-download protocol end-to-end tests passed four in `42f2f7c8-8443-46fd-8ca9-01e55074a819`; authenticated
endpoint/native-save tests passed seven in `4fdb34e2-d6ba-466f-8a74-bc32ab928a2a`. These cover resolution, symlinks,
kind/permission/size refusals, growth at a credit boundary, restricted-agent refusal, cancellation and channel reuse,
bounded replies, device authentication, desktop CORS, unused names and staging cleanup.
`cargo clippy -p farhelm-proto -p farhelm-supervisor -p farhelm-helm --all-targets -- -D warnings` and the later
helm-only all-target lint passed, as did `cargo check -p farhelm-ui --features desktop` and
`cargo check -p farhelm-desktop`.

`cd crates/farhelm-ui/js-tests && node --test` discovery initially passed 206 cases and failed the new spinner's
animation contract. After correction, focused file/link/CSS cases passed 37 in `1dca4eba-0a41-480c-934e-0260328ed3af`,
and final URI negatives passed four in `c8bfc51b-3b41-4444-9ad5-f83d161e7ec2`. The earlier successful unrelated cases
are reused because their source did not change. Cargo and release web builds passed. All eight new browser scenarios
passed on Chromium and WebKit in `b73b3bba-6b5d-4d72-b9ce-922c818f20d8`; all 28 existing web-link scenarios passed in
`3872b9b8-455e-462d-96b8-eb56d209b21b`. They cover real local/SSH bytes, checking and click refusal, fresh and cancelled
hovers, OSC file saves, symlink-sensitive paths, failure after a file vanishes, forbidden schemes, wrapping, replay,
selection and resizing. The manual Linux native attempt passed in `6c7e190d-ff49-44c6-b77e-adf72d92fc2d` with separately
inspected screenshots and saved bytes.

The docs website `bun install --frozen-lockfile && bun run build` passed from a private source copy; a pre-existing
generated-file ownership issue in the checkout was not changed. Clean bundle comparison passed all 22 requested desktop
assets. Formatting, targeted Markdown dprint, changelog format and the isolated test-delay checker passed before rebase.

The careful rebase onto `3c434a81` preserved all plan edits alongside sidebar resizing, compact approval cards and
timer-owned supervisor work. The file protocol retained version 44, both asset registrations survived, and stored
session launch directories retained their meaning. Post-rebase protocol run `7811524d-67a9-49a9-b919-ba80fa5ba65c`
passed all four cases; 361 unrelated cases were selected out, with no runtime substrate skip. This checks the actual
file dispatcher against the updated supervisor. Cargo and release web builds passed again.
`cargo clippy -p farhelm --bins -- -D warnings` passed with test seams disabled, covering the configuration earlier
all-target lint did not inspect. `cargo fmt --all -- --check`, targeted `dprint check`,
`python3 releasing/check-changelog.py format` and the isolated `python -B scripts/check-test-sleeps.py` passed after
rebase; the last inspected 277 delays with zero missing rationales. Subsequent main changes inspected so far were
queue/report metadata only.

The manual browser attempt passed in `57606a8d-fabc-4d85-8a6b-c5b00995d7db`, with local and SSH byte equality and
visually inspected hover screenshots after actual sidebar drags. The preliminary attempts failed private readiness and
drag-coordinate assumptions before any file save; their records remain retained.

Browser child commands were `cd e2e && npx playwright test terminal-files.spec.ts`, the corresponding
`terminal-links.spec.ts` selection, and the final two-file selection with a name filter for all file cases and the
wrapped-link resize case. Manual child commands are recorded as `node <owned browser attempt>` and
`python3 <owned native attempt>` here; the plan's working log preserves their private script identities.

Post-rebase Chromium/WebKit run `8490694e-ba4f-4297-9b55-4e830927145a` passed all ten selected cases: the eight file
scenarios and the two existing wrapped-link resize cases. This closes the grid/asset/layout integration risk introduced
by the new sidebar. The other 26 earlier web-link passes are reused because link parsing, selection and opening behavior
were preserved and the rebase did not alter them. A portable retained-run summary was archived privately; generic JS and
manual attempts have no structured case counts, so their stated outcomes come from retained console output and the
separately inspected artifacts rather than an aggregate case denominator.

Failed or incomplete records remain retained: initial Rust fixture compile failures, endpoint fixture label/origin and
unanswered-request failures, the spinner assertion, browser prerequisite refusals and a host-response-envelope fixture
error. Later passes are separate evidence. The first native attempt lacked embedded UI assets and proved no download;
the corrected build was used for the successful attempt. A private manual-browser probe used a nonexistent readiness
endpoint and was corrected to the public page. These were same-session development or prerequisite failures, not latent
flakes, and no overall pass is claimed for them.

The full Rust and browser batteries, doctests, installer, provisioning, release checks and hosted CI were skipped:
targeted cases cover the changed contracts and identified interactions; no executable examples, installer or release
procedure changed. No release, website deployment or live-install mutation was requested or performed.

## Review gate outcome

Each code PR passed the demanded fresh-context gpt-6.1-sol high review for correctness, design and language idiom, with
the full test-authoring checklist. The review fixes addressed bounded replies and channel reuse; native staging privacy
and cancellation ownership; parser scheme/URI boundaries, tooltip ownership and hover readiness. Scope review found no
unnecessary mechanism after correction. The final API-envelope fixture follow-up had no findings. Every touched file
received a separate documentation pass. Commit/PR wording passed fresh gpt-6.1-sol medium cold reads. Implementation,
source inspection and execution evidence belong to the executor; review agents ran no tests or VCS operations. Native
usage counters and exact runtime-model reporting were unavailable.
