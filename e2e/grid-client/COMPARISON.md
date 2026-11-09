# Grid-stream comparison, 2026-11-09 UTC

The original all-six memory target was not met: typical Chromium settled memory is essentially unchanged. Colored output
uses substantially less memory, and all six median attach sizes and times are lower. The typical Chromium RSS-growth
median is 30,617,600 bytes on main and 30,457,856 bytes on the candidate, a 0.52% difference inside the observed ranges
(30,138,368–30,732,288 and 29,851,648–31,952,896 bytes). That is not evidence of a clear memory reduction.

The maintainer accepted that result on 2026-11-09: the goal is cutting the extremes, and the colored-history memory
reductions, smaller transfers and faster attaches meet it. Typical memory is treated as essentially unchanged; no
further typical-case storage experiment is required. The plan's recorded Decision accepts this exception without
changing the measurements or the correctness requirements.

These are Linux x86-64 content-process measurements with Chromium 156.0.8078.4, WebKit 27.2, Playwright 1.64.0 and Node
26.13.0. Each case has 12,000 history lines, 200 columns and 60 screen rows. Three serial rounds alternated main and
candidate, with a fresh browser for each case: 36 successful observations. Every attach reached EOF with all history
drained, the expected dimensions, no protocol error and the same owned content-process identity through measurement. No
build or other browser test from this run competed with these captures; the shared machine's load still varied.

RSS growth follows a GC request and the fixed settling observation; it includes native allocations and allocator
retention. Peak means lifetime VmHWM at drain minus baseline RSS, and can include a pre-attach highwater. None of these
samples had the pre-attach highwater remain the maximum. This is neither a JavaScript live-heap measurement nor a Mac
WebKit result. [BENCHMARK.md](BENCHMARK.md) defines the unchanged measurement and workloads.

## Medians

Memory is MiB (1,048,576 bytes); attach size is exact grid payload bytes, excluding the private frame envelope.

### Main

| Engine   | Stream     | RSS growth, MiB | Peak above baseline, MiB | Attach bytes | Attach ms |
| -------- | ---------- | --------------: | -----------------------: | -----------: | --------: |
| chromium | typical    |           29.20 |                    45.77 |      3374337 |     388.4 |
| chromium | 256-color  |          185.29 |                   464.82 |     53416455 |    7629.3 |
| chromium | true color |          313.78 |                   612.01 |     66544524 |   13475.8 |
| webkit   | typical    |           63.15 |                    66.71 |      3374337 |     521.0 |
| webkit   | 256-color  |          727.05 |                   726.73 |     53416455 |    4506.0 |
| webkit   | true color |          896.21 |                   953.18 |     66544524 |   10469.0 |

### Candidate

| Engine   | Stream     | RSS growth, MiB | Peak above baseline, MiB | Attach bytes | Attach ms |
| -------- | ---------- | --------------: | -----------------------: | -----------: | --------: |
| chromium | typical    |           29.05 |                    40.08 |      2836127 |     257.1 |
| chromium | 256-color  |          101.71 |                   191.42 |     31459345 |    1270.7 |
| chromium | true color |          100.33 |                   195.80 |     31459345 |    1248.6 |
| webkit   | typical    |           59.68 |                    62.21 |      2836127 |     363.0 |
| webkit   | 256-color  |          265.86 |                   348.27 |     31459345 |    2637.0 |
| webkit   | true color |          296.90 |                   341.43 |     31459345 |    3294.0 |

## Individual captures

Each cell contains rounds 1, 2 and 3 in order, rounded only for display. The medians above use the original readings.

| Source    | Engine   | Stream     | RSS growth, MiB        | Peak above baseline, MiB | Attach ms                |
| --------- | -------- | ---------- | ---------------------- | ------------------------ | ------------------------ |
| main      | chromium | typical    | 29.20, 28.74, 29.31    | 44.50, 49.29, 45.77      | 388.4, 506.5, 148.0      |
| candidate | chromium | typical    | 28.47, 29.05, 30.47    | 40.08, 41.72, 38.54      | 264.0, 168.4, 257.1      |
| main      | chromium | 256-color  | 202.64, 185.29, 152.90 | 503.03, 464.82, 430.98   | 7629.3, 9620.4, 2844.3   |
| candidate | chromium | 256-color  | 100.75, 101.71, 102.47 | 190.72, 191.42, 191.96   | 1270.7, 1274.1, 1095.9   |
| main      | chromium | true color | 314.43, 313.78, 312.71 | 612.01, 614.27, 560.38   | 13475.8, 18401.1, 9138.9 |
| candidate | chromium | true color | 101.79, 99.65, 100.33  | 198.63, 195.41, 195.80   | 1356.2, 1248.6, 1008.0   |
| main      | webkit   | typical    | 63.15, 67.64, 59.36    | 66.50, 70.91, 66.71      | 521.0, 653.0, 293.0      |
| candidate | webkit   | typical    | 61.55, 59.68, 58.90    | 62.21, 62.13, 62.73      | 520.0, 363.0, 329.0      |
| main      | webkit   | 256-color  | 727.05, 731.64, 678.89 | 726.73, 730.76, 678.57   | 4315.0, 7806.0, 4506.0   |
| candidate | webkit   | 256-color  | 265.86, 304.00, 256.90 | 291.00, 350.27, 348.27   | 2613.0, 2933.0, 2637.0   |
| main      | webkit   | true color | 896.21, 948.28, 842.94 | 1014.13, 953.18, 893.13  | 10469.0, 13968.0, 9321.0 |
| candidate | webkit   | true color | 300.71, 287.79, 296.90 | 341.43, 336.01, 344.70   | 3763.0, 2249.0, 3294.0   |

## Provenance and attempts

Main assets were captured at b80071ca40246ecd39b0f1bb126aecdff4365437. Candidate assets were captured under source label
51f49e0fcc2c7358396b5563e780e8ebe666680b. The first subsequent rebase added only queue/report bookkeeping. A later
integration onto main at 1cf6e79af4548fa9158ee0f12dff6348cccdbc99 brought scoped cleanup and per-host checkout settings;
the measured assets, generator, workload, grid producer and its dependencies remained unchanged. These are reused
observations of the recorded sources, not new measurements of that integration. The only served assets that differ
between sources are listed below; all other served asset hashes match. Every round used identical assets and input
hashes for its source, and the same driver and page hashes for both sources.

| Asset            | Main SHA-256                                                     | Candidate SHA-256                                                |
| ---------------- | ---------------------------------------------------------------- | ---------------------------------------------------------------- |
| grid-model.js    | 2cef18671ae315975d4df4037e3ff3170827a6a6e73d3c6935ac3316e1170711 | 264734896b0046de10c66ddaf641bb99b7b76a02325d6ca8dd6b0f44c874829d |
| grid-terminal.js | b52bbc69690cef8c8f1948e9fd4204d0a6b4c1b61d2f19e2137a82a516e09f3f | d5d11d02d6c6a2fdf693ca1f199e8698bc260567c06ad71d685a602c8043cedf |

The production Rust generator supplied the protocol-70 main inputs before any encoding changes, and the protocol-71
candidate inputs after the binary encoding change. The incoming-main diff left the generator, workload, grid producer
and its dependencies unchanged, so those exact inputs were reused with fresh main assets. Storage-only changes did not
change candidate inputs. The driver rechecked every input hash before serving it. Driver SHA-256:
a3a19ca1bb57dc3599ab4f5dd7762a3b01fb718a000128536fdd1d0b6af3d8c1.

Recorder runs, in capture order:

- Main 1: 0f188160-9c78-46f1-b27d-e3183fb1b81a; candidate 1: 12484986-7323-4788-b626-4afea16cb8fe.
- Main 2: 67b10264-1870-41e4-920c-a4a81936b3d2; candidate 2: f019e04e-84f4-4385-ac55-5c0ecb6ea3aa.
- Main 3: 5d8390e6-b0d3-45cf-bd17-36da5b84e947; candidate 3: db337122-6fae-4f25-8dd1-b7805656c5c2.

The first storage attempt still decoded temporary strings during validation and missed memory and time targets. A
profile counted 2,412,002 decoder calls for the colored workload. Validating printable ASCII directly from bytes reduced
that to 12,002 calls while retaining fatal UTF-8 fallback for all other text. A three-round browser comparison then
improved bytes and time in all cases but still left typical Chromium memory unchanged. Retaining each exact body buffer
without a permanent byte-view wrapper removed another object per line; the tables above are that final attempt. The Node
diagnostic profiles explain those attempts but are not substitutes for these browser measurements. No cache,
compression, new allocator, cap change or drawing change was introduced.
