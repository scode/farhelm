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
