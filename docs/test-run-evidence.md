# Test run evidence

`scripts/record-test-run.py` runs one command and leaves a private, bounded record of what ran, where it ran, which
source tree it saw, and how the process ended. It is meant to provide input for later run tracking and flake hunting. It
is not a test scheduler, retry loop, pass/fail counter, or claim that a label makes a run reproducible.

NOTE: The command argv and combined stdout/stderr are retained exactly. Do not put credentials in argv. Output is
private local evidence, not publication-ready material; review it before copying any part of a run directory elsewhere.

## Running a command

Use a Unix Python interpreter with `os.waitid` and `WNOWAIT`. On macOS this requires Python 3.13 or newer; older
interpreters are refused before any child starts. The unreaped child is what keeps group cleanup tied to the original
process identity. Release setup selects a compatible interpreter explicitly.

The recorder requires a run kind, selection label, concurrency label, and an explicit `--` before the one command argv:

```console
PATH="$(scripts/build-pinned-tmux-ci.sh):$PATH" python3 scripts/record-test-run.py \
  --kind repetition \
  --selection 'session_lifecycle::create_attach_and_roundtrip_input' \
  --concurrency 'one test process' \
  --timeout 600 \
  --tmux required \
  -- cargo test -p farhelm --test e2e session_lifecycle::create_attach_and_roundtrip_input -- --exact --show-output
```

The labels are recorded verbatim. The recorder does not parse test counts or infer coverage from them. It invokes the
argv directly with the caller's current directory as the child's current directory. There is no shell joining, retry, or
scheduling layer.

`--kind` accepts `development`, `repetition`, or `release`. `--timeout` is an optional finite positive number of seconds
and starts after the command is spawned; metadata probe time is separate. `--tmux` accepts `warn` (the default),
`required`, or `none`. `--output-root` overrides evidence storage. `--keep-farhelm-env NAME` may be repeated, but each
name must begin with `FARHELM_` and must not contain a value.

`--help` and `-h` work without the required labels or a command. Help after the `--` boundary belongs to the child argv.

The default root is `$XDG_STATE_HOME/farhelm-test-runs` when `XDG_STATE_HOME` is set, otherwise
`~/.local/state/farhelm-test-runs`. This is deliberately separate from live product state. The recorder rejects roots
equal to or below the tested Git checkout, the conventional `~/.local/state/farhelm` tree, or `$XDG_STATE_HOME/farhelm`
when XDG state is configured. A newly created root and every run directory use mode `0700`; private files use mode
`0600`. An existing root must have no group or other permission bits, and the recorder does not change its owner
permissions.

Every invocation gets a full UUID directory. A retry always creates another directory, so a later passing run cannot
overwrite the failure that prompted it. The path is written to stderr as `test-run evidence: PATH` once the directory
exists, including when required tmux validation refuses the command.

## Exit status and lifecycle

The manifest's `outcome` is one of:

- `running`: the initial durable record. Seeing this after the recorder process is gone means it could not finalize, as
  with SIGKILL or host loss.
- `completed`: the child exited normally or was signaled without the recorder itself receiving an interrupt.
- `refused`: a pre-spawn policy check, currently required tmux validation, rejected the run.
- `timed_out`: the command exceeded `--timeout`.
- `interrupted`: the recorder received SIGINT or SIGTERM and forwarded it to the command group.
- `recorder-error`: spawning, evidence IO, or lifecycle management failed, so an ordinary command result would not be
  trustworthy.

Normal child exit statuses pass through unchanged. A child killed by signal `N` returns `128 + N`. Refusal and recorder
error return 125; timeout returns 124; recorder interruption returns `128 +` the received signal.

The command starts in a new POSIX process group. On recorder interruption or timeout, the recorder forwards the signal,
waits two seconds, then sends SIGKILL to its owned group. Pipe EOF never substitutes for child exit. If the group leader
exits while descendants retain the output pipe, draining ends after two seconds and owned remnants are killed. A final
half-second drain follows cleanup. The `recorder.forced_cleanup` and `recorder.cleanup_limit` fields say what happened.
A descendant that creates another session has escaped the owned group; the recorder bounds its pipe drain and discloses
that such a process may remain, but cannot claim to clean it up.

The post-kill deadline also bounds waiting for a leader that remains alive: a runnable recorder closes its pipes,
attempts a bounded final wait, and records incomplete cleanup instead of polling forever. It cannot force a process out
of an uninterruptible kernel wait. Losing child wait ownership records a recorder error; unavailable child exit status
stays null rather than being inferred as success.

SIGKILL cannot be handled. If it kills the recorder, `manifest.json` remains at its last complete atomic update (usually
`running`), and output already written to chunk files remains available. This does not promise cleanup of the child
group, crash-safe filesystem durability, or cleanup of descendants that escaped the group.

## Manifest schema 1

Git top-level discovery runs first because the recorder needs it to reject evidence storage inside the tested checkout.
Once that storage-safety preflight succeeds or records that Git is unavailable, `manifest.json` is written before the
full source capture, tmux probes, or child spawn and replaced atomically as evidence changes. Consumers must check
`schema_version` before interpreting fields and must check component `complete` flags before using fingerprints.

The top-level fields are:

- `schema_version`: integer `1`.
- `run_id`: the full UUID used as the directory name.
- `outcome`: one of the lifecycle states above.
- `started_at` and `finished_at`: UTC RFC 3339 timestamps. `finished_at` is null while running.
- `duration_seconds`: total monotonic duration, or null while running.
- `command`: exact string `argv`, actual caller/child `cwd`, requested `timeout_seconds`, and terminal
  `duration_seconds`.
- `labels`: caller-supplied `kind`, `selection`, and `concurrency`, plus a reminder that they are descriptive.
- `environment`: locale identity and FARHELM variable-name handling.
- `platform`: OS release, machine architecture, logical CPU count, and Python identity. Processor probing is omitted
  because it can launch an auxiliary subprocess outside the bounded probe lifecycle.
- `source`: Git checkout evidence and its limits.
- `tmux`: expected and actual substrate evidence, or an explicit `none` record.
- `output`: retention policy, byte counters, and ordered files. It is null before command output storage starts.
- `console`: observed, forwarded, rejected, and pending/dropped byte counters for best-effort console streaming.
- `child_status`: Python's raw return code plus normalized `exit_code` or `signal`.
- `recorder`: recorder exit code, forced-cleanup flag, cleanup limitation, and error text.

Manifest updates use a mode-`0600` temporary file, a complete write, `fsync`, and atomic replacement. This prevents
readers from seeing half-written JSON. It does not promise that the newest directory entry survives power loss.

Here are selected fields from a small successful record. Digests and detailed probe objects are shortened, and the
environment, platform, console, and repetitive probe fields are omitted from this example:

```json
{
  "schema_version": 1,
  "run_id": "00e0f0db-23d9-4937-a54e-d848f84dc62f",
  "outcome": "completed",
  "started_at": "2026-09-06T06:00:00.000Z",
  "finished_at": "2026-09-06T06:00:01.250Z",
  "duration_seconds": 1.25,
  "command": {
    "argv": ["python3", "-m", "unittest"],
    "cwd": "/path/to/checkout",
    "timeout_seconds": 120.0,
    "duration_seconds": 0.75
  },
  "labels": {
    "kind": "development",
    "selection": "Python recorder fixtures",
    "concurrency": "one process",
    "interpretation": "descriptive labels supplied by the caller; no test counts inferred"
  },
  "source": {
    "complete": true,
    "head": "0123456789abcdef0123456789abcdef01234567",
    "fingerprint_sha256": "...",
    "tracked_diff": { "complete": true, "bytes": 0, "sha256": "...", "probe": {} },
    "porcelain": { "complete": true, "bytes": 0, "sha256": "...", "probe": {} },
    "untracked_tree": { "complete": true, "entry_count": 0, "sha256": "...", "probe": {} }
  },
  "tmux": { "mode": "none", "checked": false, "uses_tmux": false },
  "output": {
    "limit_bytes": 8388608,
    "observed_bytes": 3,
    "retained_bytes": 3,
    "omitted_bytes": 0,
    "truncated": false,
    "files_in_read_order": [{ "name": "output-head.log", "bytes": 3, "role": "head" }]
  },
  "child_status": { "raw_returncode": 0, "exit_code": 0, "signal": null },
  "recorder": { "exit_code": 0, "forced_cleanup": false, "cleanup_limit": null, "error": null }
}
```

## Environment evidence

The child environment is copied from the recorder, then every `FARHELM_*` variable is removed except names supplied with
`--keep-farhelm-env`. An absent requested name remains absent. The same controlled environment is used to resolve and
run tmux.

The manifest records only FARHELM variable names: `ambient_names`, `requested_names`, `retained_names`, `removed_names`,
`requested_but_absent_names`, and the resulting `child_names`. It never records these values or hashes of them. Locale
evidence records `LANG`, `LC_ALL`, `LC_CTYPE`, Python's preferred encoding, and filesystem encoding.

## Source identity

Git top-level discovery begins at the caller's current directory. Once found, every remaining source probe runs at that
top level, so invocation from a nested directory still describes the entire checkout.

`source` contains these components:

- `head` and `head_probe`: the exact 40-digit commit identity and bounded probe evidence.
- `tracked_diff`: SHA256 and byte count of the raw `git diff --no-ext-diff --no-textconv --binary HEAD --` stdout. Raw
  diff bytes never enter metadata.
- `porcelain`: SHA256 and byte count of the complete NUL-delimited porcelain stream plus a bounded base64 prefix. Base64
  preserves invalid-byte paths without decoding them.
- `untracked_tree`: a deterministic aggregate over byte path, filesystem type, size, and the content digest or
  symlink-target digest for every nonignored untracked entry. Symlinks are not followed.
- `fingerprint_sha256`: a digest binding HEAD and the three component digests. It is present only when every component
  is complete.

The untracked aggregate processes Git's byte-sorted path stream in order. For each entry it hashes an unsigned
eight-byte big-endian path length, the raw path, a one-byte type-name length, the ASCII type name (`file` or `symlink`),
an unsigned two-byte big-endian permission mode (`stat.S_IMODE`), an unsigned eight-byte big-endian content length, and
the 32 raw bytes of the SHA256 content or link-target digest. The overall fingerprint hashes four ASCII records in this
order: `head`, `tracked_diff`, `porcelain`, and `untracked_tree`. Each record is `label`, NUL, lowercase hexadecimal
digest, NUL. These encodings are part of schema 1.

This identifies the working files a normal build reads, within Git's tracked/nonignored selection. It does not hash
ignored build artifacts or staged blob contents separately: two different index contents can share a fingerprint when
the working files and porcelain status are identical. A command that consumes the index or ignored files needs separate
evidence for those inputs. Capture is an observation of a mutable tree, not a filesystem snapshot; avoid source edits
while collecting a run whose input identity matters.

Probe stdout and stderr are streamed, hashed, and represented by bounded samples. Each probe has a five-second deadline
and a bounded post-leader pipe drain. Untracked enumeration retains no unlimited per-file list: it records counts and at
most 32 problem samples. A 100,000-entry cap and a 1 MiB unterminated-path buffer cap protect memory. Unreadable files,
unsupported types, invalid HEAD output, a failed or timed-out probe, an entry cap, or truncated inherited probe pipes
makes source capture incomplete and suppresses the overall fingerprint. Git failure does not by itself prevent the
requested local command. A probe whose child wait ownership is lost or whose leader cannot be reaped after cleanup sets
`lifecycle_complete` to false. Its bounded output and any previously observed status remain in the manifest, along with
the other source components; the recorder then exits 125 before starting the requested command. This error takes
precedence over simultaneous interruption, which remains recorded on the affected probe. Pending interruption prevents
new probe children from starting.

The file-content and executable hashing routines check their deadline between reads. Standard-library file IO cannot
interrupt a filesystem syscall that blocks inside the kernel, so this is not a hard wall-clock bound for a broken
filesystem.

## Tmux evidence

`--tmux required` checks both identities before child spawn:

1. Resolve `tmux` through the controlled child `PATH`, run that exact executable with `-V`, and compare its output with
   the `TMUX_VERSION` value parsed as plain text from `.github/release/source-pins.env`.
2. SHA256 the resolved executable and compare it with the repository-built `.ci-tmux/tmux` executable.

The source-pin file is never executed. Its source archive checksum is not treated as a binary hash. Missing files,
failed probes, unreadable executables, invalid pins, version drift, and binary drift all refuse required mode before the
child starts.

`--tmux warn` records the same evidence and prints every mismatch, then runs the command. `--tmux none` records
`checked: false` and `uses_tmux: false`; use it only when the recorded check does not use tmux. The recorder never
builds or replaces tmux.

## Output records

Stdout and stderr are combined in arrival order, streamed to stdout on a best-effort basis, and retained with an 8 MiB
payload cap. `output-head.log` holds the first 1 MiB. Up to seven `output-tail-NNNNNN.log` files hold the newest seven 1
MiB chunks; older tail chunks are removed as new chunks roll in. Writes go directly to the OS as data arrives, so useful
early and late evidence can survive recorder SIGKILL.

Read files in `output.files_in_read_order`. The manifest records `observed_bytes`, `retained_bytes`, `omitted_bytes`,
and `truncated`; chunk file names and sizes make every retained range and omission visible. Manifest size is outside the
payload cap but its samples and problem lists are independently bounded.

Console output is not authoritative. A bounded queue and raw-descriptor daemon writer keep a slow or broken consumer
from disabling timeout and evidence retention. `console.dropped_or_pending_bytes` can therefore be nonzero even when all
bytes are present in the private output chunks.
