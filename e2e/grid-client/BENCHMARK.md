# Grid-stream memory and attach benchmark

This opt-in Linux tool measures one mounted grid terminal in a fresh Chromium or WebKit content process. It reports
resident memory, lifetime highwater, encoded attach bytes and time until the history is drained. It has no performance
thresholds and normal component tests never discover it. It does not launch Farhelm or measure the helm, SSH or
emulator.

Generate inputs from the checkout being measured, in release mode, before running the driver. The output directory must
be new; the example refuses to overwrite comparison inputs. Install the e2e dependencies and Chromium/WebKit browsers as
for the component suite. Use Node 22.6 or newer for its TypeScript-stripping option; the driver resolves the installed
Playwright version from `e2e`, so no second browser/runtime dependency is added.

```sh
cargo build --release -p farhelm-supervisor --example grid_stream_bench
target/release/examples/grid_stream_bench <new-input-directory>
python3 scripts/record-test-run.py --kind development --tmux none \
  --selection 'grid attach benchmark; both engines, three workloads' \
  --concurrency 'one fresh browser at a time; no retries' -- \
  node --experimental-strip-types e2e/grid-client/benchmark.mts \
  --root <checkout> --input <input-directory> --label <source-revision> --output <new-result-file>
```

`--engine chromium|webkit|all`, `--scenario typical|indexed|truecolor|all` and `--repeats N` select a narrower
measurement or repetitions. Defaults are both engines, all workloads and one repetition. A result file must not exist.
Successful samples are saved after each case; on failure the same file also records the failure, and the command exits
nonzero. Preserve that recorder run before retrying. The pure helper checks run through the recorder too:

```sh
python3 scripts/record-test-run.py --kind development --tmux none \
  --selection 'grid benchmark framing and attribution helpers' --concurrency 'node test runner' -- \
  node --experimental-strip-types --test e2e/grid-client/benchmark.test.mts
```

## What the workloads contain

Every attach has a 200-column, 60-row screen and 12,000 history rows. Typical rows contain a changing line number,
ordinary text and three styled runs. Indexed rows give every cell a foreground palette color different from its
neighbor; truecolor rows give every cell a different RGB foreground across the whole stream. Backgrounds stay default.
These are synthetic cells, packed by the real `grid_output` functions and encoded by `GridOutput::encode`; there is no
benchmark copy of the wire serializer. The generator validates every row's shape and every emitted message's stream
order.

Each input file contains big-endian 32-bit payload lengths followed by raw grid messages. That private envelope lets the
same browser driver consume either encoding. Its SHA-256 manifest records scenario, frame count and encoded bytes. The
server streams the file from disk over its own loopback HTTP origin, and the page feeds complete payloads incrementally
to the shipped grid component. It never receives the whole attach through a Playwright evaluation. Attach bytes exclude
the private length prefixes, HTTP headers and assets. Timing includes this fixed transfer harness and ends when EOF has
arrived, all 12,000 history lines are applied, no history is pending and the screen has the expected size. It does not
call `snapshot()` or decode history to find completion.

## Memory and identity

Each sample launches a new browser server, context and page. The driver walks that server's process descendants through
Linux's per-thread children files and requires exactly one content process: a Chromium renderer or WebKit's headless
`WPEWebProcess` (the headed port calls it `WebKitWebProcess`). Chromium's spare renderer is disabled for this one-page
fixture. Missing or ambiguous content processes fail; another process's memory is never included. Kernel starttime and
PID must stay unchanged through the reading, and both are recorded in the result.

The baseline comes after assets and both font weights load, the grid mounts, a GC request completes and three animation
frames pass. After attach completes, the same GC request and fixed three-frame observation precede the final reading.
Playwright requests garbage collection but does not guarantee every unreachable object is collected. RSS is process
resident memory, including native allocations and allocator retention, rather than JavaScript live-heap size. The
observation never waits for RSS to fall to a favorable value. GC and this settling observation are outside attach time.

`before.rss` and `after.rss` are `/proc/<owned-pid>/status`'s VmRSS in bytes; `rss_growth` is their difference.
`before.hwm`, `atDrain.hwm` and `after.hwm` are VmHWM, with `atDrain` read before the final GC request.
`peak_above_baseline_rss` subtracts baseline RSS from the lifetime highwater at drain. A browser's startup can set a
higher peak before attach: `preattach_peak_not_exceeded` flags a result whose highwater did not rise beyond its baseline
value. That result bounds the attach transient but does not identify its exact peak. No kernel memory accounting is
reset. Raw readings remain alongside the derived numbers.

Results include the supplied source label, driver/page and all served asset hashes, generator metadata/input hashes,
Node/Playwright and browser versions, content-process identity and small pre/post host-load and free-memory
observations. No result claims to be the maintainer's Mac WebKit; Linux engine/build results are compared to one
another.

## Comparing revisions

Keep the driver unchanged. Pass each revision's checkout as `--root` and inputs generated by that revision's compiled
example as `--input`; the label is an explicit source identity, not a revision inferred from a directory. Build the
matching example again when its source or encoder changes. The recorded hashes let a review distinguish the inputs and
assets actually used from a claimed source label.

Run baseline and candidate interleaved, several times each, with a new result file for each driver invocation. Compare
medians per engine and workload, retaining every raw sample and the load observations. Run one case at a time; parallel
browser samples would compete for resources and distort the comparison. Measure the baseline before editing the
encoding. Do not change fixture content, settling rules or transport to make the candidate pass.

The [recorded binary-stream and compact-history comparison](COMPARISON.md) includes both engines' medians, individual
captures and source identities. Typical Chromium settled memory remains essentially unchanged, missing the original
all-six memory target. The maintainer accepted that exception on 2026-11-09 because cutting the colored-history
extremes, with smaller transfers and faster attaches, meets the goal; the comparison records that decision and the
unchanged data.
