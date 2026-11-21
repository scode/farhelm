# Runner-only termination leaves the preliminary Cargo build unsupervised

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

Stopping only the installed-agent runner during the preliminary build can leave Cargo running.

## Details

scripts/run-harness-drift.py:193–196 runs build and discovery before entering any recorded case. :43–52 and :70–74 use
direct subprocess.run calls. The runner's SIGINT/SIGTERM forwarding handlers are installed only inside recorded_case at
:129–145 and restored at :164–169. The source tests at scripts/test-run-harness-drift.py:167–195 exercise runner-only
termination through recorded_case and do not cover either preliminary child. On Linux, SIGTERM addressed only to the
runner with the default signal disposition terminates that process without delivering the signal to its Cargo child. A
process-group cancellation has a different delivery path. Extend the runner's signal ownership over the preliminary
children so a runner-only cancellation also reaps its build. The build precedes vendor sessions, and the pinned range
does not change the runner afterward.

Confidence: confirmed by inspection for the named premise of runner-only SIGTERM with the default disposition during
build. Runtime duration and actual resource usage were not measured.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `driftRunner/l16`.
