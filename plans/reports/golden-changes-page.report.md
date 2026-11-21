## What this was about

Reviewers used to open changed golden screenshots one at a time. That made it easy to miss a change on a screen the
patch was never meant to touch. After a comparison or update, the golden runner now writes a portable page containing
only changed and new pictures. Each pair shows the reference, the new capture, and the exact changed pixels in orange
with a count. A resize shows both dimensions. The same visual review procedure now covers targeted iteration and the
full update before publishing.

## Things you should know

The page lives in `target/gui-goldens/changes/`. Copy the whole directory to share it. Use the page path printed by the
current run. A run with no differences clears the previous page. An update can use matching local references or fetch
the old pictures it needs. It still shows the captures when those references are unavailable.

The queue approved using the existing Rust image library. Comparisons reuse decoded images, and updates send only the
changed pairs that have references to the same test program after rendering. The page remains plain HTML. On Linux the
page-only timings were as follows, using four compiled workers and up to 16 Python workers.

| Workload                                            |   Python | Compiled |
| --------------------------------------------------- | -------: | -------: |
| 68 changed or new pictures, including 42 highlights |  27.76 s |   2.04 s |
| All 328 pictures changed                            | 111.95 s |   6.10 s |

## Open questions and possible follow-ups

Use the compiled path for both targeted and full reviews. The roughly six-second all-changed page benchmark ran
separately from the roughly 35-second clean full capture. The decision-revisit TODO records the choice for the
maintainer.

## Pull requests

- Build the page and its pixel evidence. [#4457](https://github.com/scode/farhelm/pull/4457/changes)
- Document the visual review procedure and remove the original TODO.
  [#4458](https://github.com/scode/farhelm/pull/4458/changes)

## Checks

- `cargo test --locked -p farhelm-gui --lib goldens::changes` passed the exact pixel and resize tests, recorder
  `c2ead7b0-1578-4ab8-b17a-bef4c6ed35c3`. The page-only benchmark ran the update page generator against retained
  captures and staged references. It invoked the real compiled worker and produced the expected rows and highlights,
  recorder `11230b2b-6d75-49fb-bc3b-7b2760787a36`. The Python timing references are
  `c24b3398-6b5c-4ce8-a005-bd67f239d7d4` and `13894420-86aa-4dca-a42e-4df9340fe35e`.
- A normal full `scripts/check-gui-goldens.py --update` on the final stack wrote and certified all 328 captures. Their
  hashes matched the manifest and produced no page, recorder `6b6fe57a-d9c4-43e8-a3e1-824a85a85aa8`.
- `python3 -B scripts/test-check-gui-goldens.py` passed 21 tests, including changed update pairs, offline update,
  portable output, a moving manifest, and interrupted render or review, recorder `d20c58e0-6c71-42cc-a957-462cb80a5a9a`.
- A targeted real comparison kept its expected failure status on mismatched and missing references and wrote four rows
  and three native highlights, recorder `8e69ccb4-fa64-4553-bf6a-c1642d3b9086`. Each highlight matched the original
  Python output pixel for pixel. A clean full comparison passed and cleared the page, recorder
  `95a43d6f-7ba8-4f49-9060-3f8c575cd4bb`. The final targeted run displayed an invalid PNG as unavailable, recorder
  `42927f88-ff54-4608-aaa0-6cf82cb0d352`. A succeeding clean targeted run removed that page, recorder
  `ba02d458-dbe1-4731-b55b-19bfec82db83`.
- Formatting, Markdown references, changelog validation and the Rust test delay check passed. The Markdown check has one
  existing warning in the triage archive.
- Reused the initial round's clean full update as the timing baseline and source of identical inputs for both
  benchmarks, recorder `b87a9294-adaa-45de-8bd0-2cb9a673e592`. Reused the store self-tests, recorder
  `81905289-aafd-490e-9fdd-b0909e046b5b`, and the Chromium layout check, recorder
  `16c510fc-6827-4cdc-b717-142ddaafea28`, from the Python implementation based on `7b78f1c7b941`. The store code, HTML
  layout and decoded highlight pixels still match that coverage. The final Linux comparison covered the new
  unavailable-reference behavior.
- Skipped the broad workspace and release suites because this is development tooling. Runtime validation ran on Linux.

## Review gate

Both drafts cleared their independent source gates after the findings were fixed and verified. Cancelling a run stops
its work before an update can receive a completion marker. The page uses the run's pinned manifest and shows invalid
references as unavailable. The procedure points reviewers to the page printed by their current run.

### Landing

#### What landed with it

The plan's two PRs merged with a landing fix, #4462, right after launcher-command-field in the same round. That plan
redesigned the session launcher and changed or added 18 golden pictures. This plan's change to the shared golden test
file is in a different function, and the two fit together. The round's full comparison rendered the launcher's new
pictures with this plan's page in place and found no differences, so no page was written. The landing therefore never
made a page itself; the executor's runs and the unit and script tests are what exercised it. An independent reviewer and
I read both PRs against the plan and the decision that answered its block. Neither found a blocker. The decision moved
the page's pixel work from Python into the compiled golden test program, and the executor followed it: the page is still
plain HTML, and both paths' timings are in the report.

The plan's speed requirement, that making the page does not noticeably lengthen a run, is met. The page took about 2
seconds for 68 changed or new pictures, against 28 seconds on the Python path. With all 328 pictures of the set before
the launcher redesign changed, it took about 6 seconds, against 112, on top of a full update that takes about 35.

#### Fixes made while landing

One, in [#4462](https://github.com/scode/farhelm/pull/4462/changes), documentation only: the validation guide now names
the page generator's two files where it says to run the runner's tests when they change, and a code comment now says a
missing reference picture is shown on the page as unavailable, the same as one that cannot be read.

#### Checks

This round landed launcher-command-field (the launcher, shared controls, a conversation-ID check moved into code the
helm, supervisor and app share, goldens, docs pages and their screenshot steps) and golden-changes-page (the golden
runner and its page). The landing ran the Rust workspace suite, the desktop app's tests, the lint runs, the end-to-end
smoke test and the two affected docs pages' screenshots on a separate arm64 Linux test machine, and the goldens, script
tests and small checks locally on x86_64.

The test machine's disk filled during the first workspace run. Its build folder for this checkout had grown to about 117
GB over earlier rounds, and the optimized development builds the earlier goldens-fast-and-targeted plan introduced
pushed it over. The test machine's script cleans that folder only when a run starts with less than 15 GB free, which was
not enough here; raising that threshold is worth a small follow-up. Four tests failed on the full disk, two of them
saying so outright, and every later run on that machine died with it. Those runs are not evidence of anything (the
workspace run is kept as `cdc3ecf6-840c-42a6-bf87-244a67a4d899`). I removed only that build folder and ran the set again
from a cold build.

On both plans with their landing fixes (#4461 for the launcher, #4462 for the golden page), after the disk was freed:

- `cargo nextest run --workspace --exclude farhelm-desktop`, on the test machine (recorder run
  `2ceaf252-220e-430b-812c-b569e2b2df91`): all 8217 passed; the 86 skipped are ignored by default.
- `cargo nextest run -p farhelm-desktop`, on the test machine (recorder run `92bb77f0-7864-41e5-8dba-5846ddb6e477`): all
  184 passed.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings` and the desktop app's own
  two clippy runs, on the test machine, not through the recorder: passed.
- `python3 -B scripts/gui-e2e-smoke.py`, on the test machine. Before the smoke's own fix in #4461, it failed two steps
  that waited for the launcher's removed header text (recorder run `b7d510cb-a992-4815-9bad-35700a12ec90`). With that
  fix (recorder run `f32d7fbe-7ded-4453-8d1d-ac6b112abfe0`), every step passed but paste, which skips on Linux as it
  always does.
- `scripts/docs-screenshots.sh --local --only start-a-session --only launch-templates`, on the test machine (recorder
  run `b5a4cbb5-678a-426a-a160-b4f9ae4f6ec5`): passed, all 18 pictures captured.
- `cargo nextest run -p farhelm-gui -E 'test(/^launcher::/) | test(/^templates::/)'`, locally (recorder run
  `73bd1438-2bb0-43ad-878c-582d0b6f5a99`): all 835 passed, the new key-cap assertions among them.
- `python3 scripts/check-gui-goldens.py --scene 'launcher-*' --scene 'template*' --scene dialog-launcher-frame`, locally
  (recorder run `2dd61402-ac9d-492e-a4df-5c7ad8bb945f`): passed.
- The website build (`bun install --frozen-lockfile && bun run build`), with its link check, locally: passed.
- `python3 -B scripts/test-gui-e2e-smoke.py`, `python3 -B scripts/test-gui-screenshot-batch.py`,
  `cargo fmt --all -- --check`, `dprint check`, `python3 scripts/check-agents-md.py` and
  `python3 releasing/check-changelog.py format`, locally: passed.

These ran on both plans as delivered, stacked, before the landing fixes. They still count: the fixes change what is
drawn only in Replace with's recent rows, which no golden picture shows, and the launcher goldens above passed after
them.

- `python3 scripts/check-gui-goldens.py`, the full comparison, locally (recorder run
  `83dbd97b-0984-4847-bd8c-3236760f48fe`): passed, with the launcher's new pictures and no changes page written.
- `python3 -B scripts/test-check-gui-goldens.py` (recorder run `83208fa7-3a0e-4caa-b2b1-de26a7508ca1`, 21 passed),
  `python3 -B scripts/gui-goldens.py --self-test`, `python3 -B scripts/test-docs-shots.py` (recorder run
  `c69c1ece-81a2-435a-829b-8f9dc45774d5`), `python3 -B scripts/test-gui-agent.py` (recorder run
  `f8be9065-0615-45ac-add4-e8bc9e0858db`, 94 passed) and `python -B scripts/check-test-sleeps.py` (in its isolated
  environment), locally: passed.

Skipped: macOS, which the launcher's report leaves to native validation and which the golden page does not affect; the
screenshot batch itself (only its script tests ran); CI, which runs only on demand.

#### Things you should know that the report does not say

- A cancelled run leaves the previous run's page in place, where it can be mistaken for the current one; any finished
  run replaces or removes it. Use the page path the current run printed.
- The visual-review procedure sits in the middle of the goldens entry of the validation guide, with no heading of its
  own, so a plan cannot cite it by section name yet.
- The data the compiled program writes for the page is tested only through stand-ins on the Python side, so a change to
  its format would pass the tests and show up only in a real comparison run.
