//! Portable process-table reads for lifecycle sweeps and hook attribution.
//!
//! `service::sweep` owns every DECISION a stop, delete, or close makes: the
//! PPID closure, the environment-marker union that finds reparented
//! daemons, start-time validation against pid reuse, the
//! SIGTERM/SIGSTOP-quiesce/SIGKILL escalation, and the confirm-gone poll.
//! None of that is platform-specific. The original sweep needs three reads:
//!
//! 1. every process this user owns, with its parent and its start time
//!    ([`snapshot`]);
//! 2. one pid's parent, start time, and whether it has become a zombie
//!    ([`read_process`]);
//! 3. one pid's exec-time environment ([`read_environ`]).
//!
//! Keeping platform reads separate is the point. Hook attribution additionally
//! inspects native executable identity through the same platform boundary.
//! Linux and macOS must run the SAME sweep rather than two sweeps that happen to agree,
//! because the promise stop and delete make to a user — "nothing this
//! session started is still running" — is supposed to mean the same thing
//! on both. Until this module existed the sweep read `/proc` inline, so on
//! macOS every enumeration failed with "reading /proc: No such file or
//! directory" and, since the sweep fails CLOSED by design, stop and delete
//! failed with it (SPEC_impl.md recorded the Mac variant as deferred; this
//! is it).
//!
//! # Contracts that hold on every platform
//!
//! **Start time is an opaque identity token, never a timestamp.** Linux
//! reports jiffies since boot (`/proc/<pid>/stat` field 22); macOS reports
//! the process's start `timeval` folded to whole microseconds. Both are
//! injective and stable for the life of a process on one boot, which is
//! all the sweep needs — it only ever compares a value from this module
//! against ANOTHER value from this module for equality. Nothing may
//! subtract them, order them, compare them across a reboot, or read a wall
//! clock out of them. The one property the platforms must genuinely share
//! is that [`snapshot`] and [`read_process`] derive the value the SAME way,
//! since pid-reuse detection compares one against the other.
//!
//! **The environment is the exec-time environment, and only that.** On
//! Linux that is `/proc/<pid>/environ`; on macOS it is the environment
//! region of `KERN_PROCARGS2`, with the argv region deliberately excluded
//! (see `parse_procargs2`). A process can be claimed by a sweep only for
//! what its parent handed it at exec, never for text it happens to have on
//! its command line. NOTE: macOS 26+ withholds that region entirely for
//! Apple platform binaries — see [`read_environ`] for the observation and
//! what it costs the caller.
//!
//! **Unreadable is not an error for environments, and IS one for
//! everything else.** A foreign-uid or hardened process makes its
//! environment unreadable as a matter of routine, so [`read_environ`]
//! answers `None` and the sweep reads that as "carries no marker" — the
//! safe direction, since a sweep never signals a process it could not
//! identify. Enumeration is the opposite: [`snapshot`] returning `Err`
//! rather than an empty map is what stops a host where the process table
//! cannot be read at all from reporting a falsely-clean sweep.

use std::collections::HashMap;

/// The only distinction the sweep draws between one live process and
/// another.
///
/// A zombie has already exited and is merely waiting for an ancestor to
/// reap it. Nothing this supervisor does can force that reap, and a zombie
/// cannot run code, so the sweep's confirmation step (`service::sweep`) counts
/// one as gone; treating it as still-alive would fail a stop for a reason
/// no amount of signaling could ever fix. Everything that is not a zombie
/// — running, sleeping, stopped, traced — is [`ProcessState::Running`],
/// because the sweep has no use for the difference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProcessState {
    /// Alive as far as this sweep cares: it may still execute code.
    Running,
    /// Exited, not yet reaped. Counts as gone.
    Zombie,
}

/// What one walk of the process table yields: pid → (ppid, start time).
///
/// A map rather than a list because the sweep's PPID closure is a
/// repeated "is this pid's parent already in the found set" question, and
/// a pair rather than a struct because these two numbers travel together
/// through the whole sweep and neither means anything without the other —
/// a ppid names an edge to walk, a start time names the identity that
/// makes acting on that pid later safe.
pub(crate) type ProcessTable = HashMap<u32, (u32, u64)>;

/// One full walk of the process table, restricted to this euid: pid →
/// (ppid, start time), plus soft per-process errors.
///
/// Fails closed. A problem that prevents the walk from happening AT ALL
/// (no `/proc` to open, a `sysctl` that refuses) is `Err`, never an empty
/// map: reporting "found nothing" when nothing was ever looked at is
/// precisely the failure mode the sweep exists to avoid, so the caller
/// must be able to tell "the tree is clean" from "the tree was never
/// examined". A problem confined to ONE process (an unreadable row on
/// Linux) lands in the returned `Vec<String>` instead, so a single odd
/// process does not blind the scan to every other one.
///
/// The scope is same-euid, mirroring what each platform can actually read:
/// another user's processes are neither killable by this supervisor nor
/// inspectable for markers, so including them would only add rows nothing
/// downstream can act on — and, on a `hidepid`-hardened Linux host, a
/// stream of permission errors for every unrelated process on the machine.
///
/// This is a plain sequential scan, not an atomic snapshot: a fork racing
/// the walk may or may not appear, and (in principle) a pid may exit and
/// be recycled while the walk is still running. That is not fixed here and
/// cannot be. The sweep compensates by re-reading both endpoints before
/// following snapshot PPID edges, re-walking between signal phases, and
/// re-checking each chosen process at the moment of signaling.
pub(crate) fn snapshot() -> Result<(ProcessTable, Vec<String>), String> {
    imp::snapshot()
}

/// One process's `(ppid, start time, state)`, or `Ok(None)` when it is gone
/// or, on Linux, when a failed read finds that the pid now belongs to another
/// uid.
///
/// Both `Ok(None)` cases mean there is no process identity this sweep may act
/// on, so neither is worth reporting. Everything else that goes wrong — a
/// permission error for a pid this supervisor is still supposed to own, a
/// malformed row, a syscall failure that is not "no such process" — comes
/// back as `Err` rather than being folded into "gone". That direction is
/// load-bearing throughout the sweep: ancestry expansion must report an
/// unreadable candidate, `signal_validated` must not silently skip one, and
/// `confirm_gone` must not count an unreadable survivor as confirmed dead.
pub(crate) fn read_process(pid: u32) -> Result<Option<(u32, u64, ProcessState)>, String> {
    imp::read_process(pid)
}

/// One process's exec-time environment as the kernel's own
/// NUL-delimited `KEY=VALUE\0KEY=VALUE\0...` block, or `None` when it
/// cannot be read.
///
/// There is deliberately no error channel. Every way this can fail —
/// the process exited, it belongs to another user, it is non-dumpable
/// (`PR_SET_DUMPABLE(0)`, or any setuid exec, which clears dumpability),
/// macOS refuses `KERN_PROCARGS2` for a process this one does not own — is
/// routine for a scan that sweeps every process on the host, and all of
/// them mean the same thing to the caller: no marker could be read here,
/// so this process is not claimed. That is the SAFE direction (a sweep
/// never signals what it could not identify) and it is also the accepted
/// residual documented on `sweep::environ_markers_of`: an
/// environment-scrubbing descendant escapes the marker scan, and only a
/// cgroup can close that.
///
/// One macOS-specific answer shape callers must know about: for an Apple
/// PLATFORM binary, macOS 26+ answers `Some` with the environment region
/// simply ABSENT (argv-only, even for a same-uid direct child — observed
/// on macOS 26.5.1, where `/bin/sleep` answered 29 bytes from a megabyte
/// buffer while a locally built child answered in full). To this module
/// that is indistinguishable from a genuinely empty environment, so it is
/// NOT collapsed to `None`; it surfaces as "no marker", and the residual
/// it creates for the sweep is documented where the sweep reasons about
/// residuals. The test pinning the behavior is
/// `a_platform_binary_childs_environment_is_withheld_on_modern_macos`.
pub(crate) fn read_environ(pid: u32) -> Option<Vec<u8>> {
    imp::read_environ(pid)
}

/// A PID paired with the kernel token that distinguishes it from PID reuse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProcessIdentity {
    pub(crate) pid: u32,
    pub(crate) start: u64,
}

impl ProcessIdentity {
    pub(crate) fn read(pid: u32) -> Option<Self> {
        match read_process(pid).ok()? {
            Some((_, start, ProcessState::Running)) => Some(Self { pid, start }),
            _ => None,
        }
    }
}

/// How far up one attribution may climb: the pane must be reachable
/// within this many live edges, or the ancestry is too deep to be the
/// foreground runtime's (and the walk refuses rather than sampling it).
pub(crate) const MAX_ATTRIBUTION_ANCESTORS: usize = 64;

/// Per-process command-line evidence budget. A process whose argv cannot
/// be captured completely within this bound contributes NO argv evidence
/// (`None`) rather than a truncated prefix a later proof could mistake
/// for the whole command line; the walk itself continues, because the
/// runtime decision for current kinds rests on the image, not the args.
pub(crate) const MAX_ARGV_BYTES_PER_PROCESS: usize = 64 * 1024;

/// Whole-walk command-line budget. Unlike the per-process bound this one
/// refuses the walk: megabytes of argv across the ancestry is not a
/// foreground runtime, it is a denial-of-observation, and accepting the
/// report without the evidence would bless exactly the gap the caps
/// exist to close.
pub(crate) const MAX_ARGV_BYTES_PER_WALK: usize = 1024 * 1024;

/// One live edge of an attribution walk: the process, its parent, the
/// kernel start token distinguishing it from PID reuse, and the image
/// and argv evidence captured while the edge was observed.
///
/// `exe` is REQUIRED — an unreadable image refuses the walk, the same
/// way it always refused the emitter check — while `argv` is `None`
/// whenever that process's command line was unreadable or exceeded its
/// per-process bound (see the constants above for why those differ).
#[derive(Debug, Clone)]
pub(crate) struct ChainLink {
    pub(crate) pid: u32,
    pub(crate) ppid: u32,
    pub(crate) start: u64,
    pub(crate) exe: Vec<u8>,
    pub(crate) argv: Option<Vec<Vec<u8>>>,
}

/// The command-line evidence for one process, bounded per
/// [`MAX_ARGV_BYTES_PER_PROCESS`]. Linux reads `/proc/<pid>/cmdline`;
/// macOS re-reads the `KERN_PROCARGS2` buffer the environ reader
/// fetches and parses out the argv region instead of the environ one.
/// Every failure — gone, foreign uid, non-dumpable, truncated,
/// over-budget — collapses to `None`: missing argv evidence is a fact
/// the per-kind proof interprets, never a walk failure by itself.
pub(crate) fn read_process_argv(pid: u32) -> Option<Vec<Vec<u8>>> {
    imp::read_process_argv(pid)
}

/// Walk the live ancestry from a hook connection's process to the owned
/// pane, capturing image and argv evidence per edge.
///
/// This is the shared mechanics every foreground-runtime proof builds
/// on: at most [`MAX_ATTRIBUTION_ANCESTORS`] live `Running` edges, the
/// peer's start token verified before anything else, loops and a missing
/// pane refused, edges plus image observations re-read before return,
/// and the whole walk repeatable by the repeat attribution before
/// commit. It decides NOTHING about vendors — the per-kind step applies
/// its own corridor and runtime recognition to the returned chain — so
/// a new proof reuses the walk rather than re-arguing liveness, PID
/// reuse, and budgets.
/// Refuse a walked chain whose command-line evidence totals past
/// [`MAX_ARGV_BYTES_PER_WALK`]. A separate pass over the captured chain
/// rather than inline accounting, so the bound reads the same evidence
/// the per-kind proofs will match against — not a parallel count kept
/// beside it — and so later proofs reuse the check instead of
/// re-arguing the budget.
fn check_walk_argv_budget(chain: &[ChainLink]) -> Result<(), String> {
    let total: usize = chain
        .iter()
        .filter_map(|link| link.argv.as_ref())
        .flatten()
        .map(Vec::len)
        .sum();
    if total > MAX_ARGV_BYTES_PER_WALK {
        return Err("the hook ancestry's command lines exceed the attribution budget".to_string());
    }
    Ok(())
}

pub(crate) fn walk_to_pane(peer: ProcessIdentity, pane_pid: u32) -> Result<Vec<ChainLink>, String> {
    let mut chain: Vec<ChainLink> = Vec::with_capacity(8);
    let mut pid = peer.pid;
    for _ in 0..MAX_ATTRIBUTION_ANCESTORS {
        let Some((parent, start, ProcessState::Running)) = read_process(pid)? else {
            return Err("the hook ancestry is no longer live".to_string());
        };
        if chain.is_empty() && start != peer.start {
            return Err("the hook connection's process identity changed".to_string());
        }
        // The image comes from the same source the emitter check has
        // always read; an unreadable one refuses the walk exactly as it
        // always refused the report.
        let exe = imp::process_exe(pid)?;
        let argv = read_process_argv(pid);
        chain.push(ChainLink {
            pid,
            ppid: parent,
            start,
            exe,
            argv,
        });
        if pid == pane_pid {
            // Re-read every edge plus every image before returning: a
            // process that exec'd between the walk and this check is no
            // longer the process the walk observed, and the report must
            // not be admitted on the earlier observation. Edges or image
            // changed both refuse; argv is per-observation evidence the
            // repeat attribution re-captures rather than an identity
            // input, so it is not compared here.
            for link in &chain {
                if read_process(link.pid)? != Some((link.ppid, link.start, ProcessState::Running))
                    || imp::process_exe(link.pid)
                        .map_err(|_| ())
                        .unwrap_or_default()
                        != link.exe
                {
                    return Err("the hook ancestry changed during attribution".to_string());
                }
            }
            check_walk_argv_budget(&chain)?;
            return Ok(chain);
        }
        if parent == 0 || chain.iter().any(|link| link.pid == parent) {
            break;
        }
        pid = parent;
    }
    Err("the hook cannot be attributed to this session's foreground pane".to_string())
}

/// Attribute a hook connection to the one Codex executable under the owned pane.
///
/// A package-manager wrapper may sit between the pane and native Codex. A nested
/// Codex has two native Codex ancestors instead and cannot report for its parent.
/// This establishes process provenance only; transcript metadata must separately
/// exclude vendor threads sharing the foreground process.
///
/// The walk is the shared [`walk_to_pane`] mechanics; the corridor applied
/// below is Codex's instance of the restrictive rule.
pub(crate) fn foreground_codex_emitter(
    peer: ProcessIdentity,
    pane_pid: u32,
) -> Result<ProcessIdentity, String> {
    let chain = walk_to_pane(peer, pane_pid)?;
    codex_corridor(&chain)
}

/// Whether raw argv spells the supported hook invocation: the installed
/// hook command's shape (`<farhelm> internal hook ...`), matched
/// syntactically, never by path — an upgraded supervisor must still
/// accept hooks installed by its predecessor, and the install path is
/// attacker-influenced anyway. `argv[0]` is the executable spelling
/// (anything); what matters is the verb sequence after it.
fn is_hook_invocation_argv(argv: &[Vec<u8>]) -> bool {
    matches!(argv, [_, internal, hook, ..] if internal == b"internal" && hook == b"hook")
}

/// Whether a `-c` command string carries executable shell syntax outside
/// string literals: separators and backgrounding (`;`, `&`), pipes
/// (`|`), grouping (`(`, `)`), redirections (`<`, `>`), substitution
/// (backquote, `$`), negation/history (`!`), comments (`#`), and line
/// breaks. Single quotes protect everything to the next `'`; double
/// quotes protect everything except a backslash escape; elsewhere a
/// backslash escapes the next character (including a newline
/// continuation). Metacharacters inside literals — notably inside a
/// quoted executable path — are path characters, not syntax, and pass.
///
/// The word splitter alone cannot carry the trampoline contract: it has
/// no operator recognition, so `<hook> <payload>; printf done` splits
/// with the hook in the first three words and looks like the supported
/// command while the shell runs a redirection and a second command.
fn has_unquoted_shell_syntax(command: &str) -> bool {
    enum Quote {
        None,
        Single,
        Double,
    }
    let mut quote = Quote::None;
    let mut chars = command.chars();
    while let Some(next) = chars.next() {
        match quote {
            Quote::Single => {
                if next == '\'' {
                    quote = Quote::None;
                }
            }
            Quote::Double => {
                if next == '\\' {
                    chars.next();
                } else if next == '"' {
                    quote = Quote::None;
                } else if next == '$' || next == '`' {
                    // Expansions stay live inside double quotes: a quoted
                    // `"$(evil)"` still executes, unlike single quotes.
                    return true;
                }
            }
            Quote::None => match next {
                '\'' => quote = Quote::Single,
                '"' => quote = Quote::Double,
                '\\' => {
                    chars.next();
                }
                ';' | '&' | '|' | '(' | ')' | '<' | '>' | '`' | '$' | '!' | '#' | '\n' | '\r' => {
                    return true;
                }
                _ => {}
            },
        }
    }
    false
}

/// A narrow trampoline: a POSIX shell directly invoking the supported hook
/// command and nothing else — the shape vendor hook runners produce
/// (`sh -c '<farhelm> internal hook ...'`). The command must be exactly
/// one simple command: any unquoted control operator, redirection,
/// substitution, or other executable shell syntax refuses, even when the
/// hook comes first. An interactive shell, a script, a chained command,
/// or any other image is an unclassified intermediary and refuses, as
/// does any other session-hosting runtime.
///
/// Argv is self-reported by the intermediary, so this classifies honest
/// trampolines; it cannot unmask a descendant that forges the exact hook
/// shape. That forgery is outside the attribution model (inherited
/// credentials are not a same-user security boundary — see the module
/// docs): the value is refusing every delegated path that does NOT
/// bother to mimic, while the resumable-or-clear gate still bounds what
/// a mimicking reporter can establish without a valid record.
fn is_hook_trampoline(exe: &[u8], argv: &[Vec<u8>]) -> bool {
    let name = exe.rsplit(|byte| *byte == b'/').next().unwrap_or_default();
    if !matches!(name, b"sh" | b"bash" | b"dash") {
        return false;
    }
    let [_, flag, command] = argv else {
        return false;
    };
    if flag != b"-c" {
        return false;
    }
    let command = match std::str::from_utf8(command) {
        Ok(command) => command,
        Err(_) => return false,
    };
    if has_unquoted_shell_syntax(command) {
        return false;
    }
    // Real shell lexing, not a substring search: `evil; <hook>` and
    // `sh -c <script>` must not pass on the strength of mentioning the
    // hook somewhere in the command string.
    match shell_words::split(command) {
        Ok(words) => {
            matches!(words.as_slice(), [_, internal, hook, ..] if internal == "internal" && hook == "hook")
        }
        Err(_) => false,
    }
}

/// Whether raw `exe` bytes name another integrated agent runtime: a
/// session-hosting image that is not the Codex emitter this corridor
/// counts. Such an intermediary means the report traveled through a
/// different harness, and refuses with its own message so the diagnostic
/// names the mechanism rather than a generic unclassified intermediary.
fn is_other_session_runtime(exe: &[u8]) -> bool {
    let name = exe.rsplit(|byte| *byte == b'/').next().unwrap_or_default();
    matches!(name, b"claude" | b"goose" | b"pi" | b"omp")
}

/// Codex's instance of the restrictive corridor, over an already-walked
/// chain: the reporter (first link, the kernel-attributed socket peer)
/// must be the supported hook invocation; exactly one native `codex`
/// image is the emitter (this accepts package-manager wrappers while
/// refusing a nested native Codex); the pane anchor (last link, the
/// owned foreground) is accepted by position; every other link must be
/// a narrow [`is_hook_trampoline`] trampoline. Any other session-hosting
/// runtime or unclassified intermediary refuses.
///
/// Pure over the chain so the shapes are unit-testable without live
/// processes; [`foreground_codex_emitter`] supplies the walked chain.
fn codex_corridor(chain: &[ChainLink]) -> Result<ProcessIdentity, String> {
    let reporter = chain
        .first()
        .ok_or_else(|| "the hook ancestry is empty".to_string())?;
    match reporter.argv.as_deref() {
        Some(argv) if is_hook_invocation_argv(argv) => {}
        _ => {
            return Err("the reporting process is not the supported hook invocation".to_string());
        }
    }
    let mut emitter = None;
    for (index, link) in chain.iter().enumerate() {
        if imp::is_codex_image(&link.exe) {
            if emitter.is_some() {
                return Err("a nested Codex process cannot report for the foreground".to_string());
            }
            emitter = Some(ProcessIdentity {
                pid: link.pid,
                start: link.start,
            });
            continue;
        }
        if index == 0 || index + 1 == chain.len() {
            continue;
        }
        let trampoline = link
            .argv
            .as_deref()
            .is_some_and(|argv| is_hook_trampoline(&link.exe, argv));
        if trampoline {
            continue;
        }
        if is_other_session_runtime(&link.exe) {
            return Err(
                "another session-hosting runtime sits between the reporter and the foreground"
                    .to_string(),
            );
        }
        return Err(
            "an unclassified intermediary sits between the reporter and the foreground".to_string(),
        );
    }
    emitter.ok_or_else(|| "the hook has no attributable Codex executable".to_string())
}

/// OMP's installation-descriptor evidence, matched over walked chain links.
///
/// The installed `omp` command is a script: the kernel runs its `bun`
/// shebang, so the live runtime image is the Bun binary and the entry point
/// is the bundle path in program-argument position. The compiled target is
/// a native `omp` image instead. Both shapes are recognized by exact
/// bundle/entry suffixes, never by basename alone.
const OMP_DIST_ENTRY_SUFFIX: &[u8] = b"node_modules/@oh-my-pi/pi-coding-agent/dist/cli.js";
const OMP_SRC_ENTRY_SUFFIX: &[u8] = b"packages/coding-agent/src/cli.ts";
const OMP_PACKAGE: &[u8] = b"@oh-my-pi/pi-coding-agent";
/// OMP's selected command under npm/npx: the package's own bin, spelled
/// exactly as the launcher resolves it. Anything else after the option
/// boundary is a foreign command the package merely makes available.
const OMP_BIN: &[u8] = b"omp";

/// The raw image basename, over bytes the caller already captured, with the
/// procfs atomic-update suffix stripped the way the emitter check strips
/// it. Shared by every OMP image test so no caller re-derives it.
fn omp_image_basename(exe: &[u8]) -> &[u8] {
    let name = exe.rsplit(|byte| *byte == b'/').next().unwrap_or_default();
    name.strip_suffix(b" (deleted)").unwrap_or(name)
}

/// Whether the image is a Bun interpreter: the only interpreter whose
/// execution of OMP is a supported shape. Node execution is unverified and
/// refused elsewhere, never inferred from a `.js` suffix.
fn is_bun_image(exe: &[u8]) -> bool {
    omp_image_basename(exe) == b"bun"
}

/// Whether the image is a Node interpreter: observed only to refuse with a
/// diagnostic naming the mechanism rather than a generic unclassified
/// intermediary.
fn is_node_image(exe: &[u8]) -> bool {
    matches!(omp_image_basename(exe), b"node" | b"nodejs")
}

/// Whether the image is a compiled OMP target: the native binary the
/// pinned `build-binary.ts` emits as `dist/omp` (or a `dist/omp-<id>`
/// cross build). A script named `omp` never matches: scripts resolve to
/// their interpreter's image before this comparison runs.
fn is_compiled_omp_image(exe: &[u8]) -> bool {
    let name = omp_image_basename(exe);
    name == b"omp" || name.starts_with(b"omp-")
}

/// Whether raw argv spells the selected OMP entry point: the installed
/// bundle or the explicitly selected source-tree `src/cli.ts`, matched as
/// a path suffix so a same-named file elsewhere is not evidence.
fn is_omp_bundle_entry(arg: &[u8]) -> bool {
    arg.ends_with(OMP_DIST_ENTRY_SUFFIX) || arg.ends_with(OMP_SRC_ENTRY_SUFFIX)
}

/// Whether a Bun launcher link spells a supported package launch: `bun x`
/// or `bunx` selecting exactly OMP's package. Only the head is
/// constrained; trailing tokens are OMP's own arguments and the runtime
/// grammar below judges them. Any other spelling — flags before the
/// subcommand, `--package` forms, package scripts — is not an observed
/// shape and refuses.
fn is_omp_bun_launcher(argv: &[Vec<u8>]) -> bool {
    match argv {
        [program, subcommand, package, ..]
            if omp_image_basename(program) == b"bun"
                && subcommand == b"x"
                && package == OMP_PACKAGE =>
        {
            true
        }
        [program, package, ..]
            if omp_image_basename(program) == b"bunx" && package == OMP_PACKAGE =>
        {
            true
        }
        _ => false,
    }
}

/// Whether a token past npx's positional package could change what the
/// launcher runs: a command-string mode in any spelling, or a package
/// selection. npx parses options across the whole argv rather than only
/// before the command, so any of these trailing the package refuses —
/// this rule does not reimplement that splitting to decide which ones
/// npx would actually consume.
fn is_npm_launcher_level_token(token: &[u8]) -> bool {
    token == b"-c"
        || token == b"--call"
        || token.starts_with(b"--call=")
        || token == b"--package"
        || token.starts_with(b"--package=")
        || token == b"-p"
        || token == b"--package-name"
}

/// Whether a Node launcher link spells a supported npm/npx launch of
/// OMP: the `npm-cli.js`/`npx-cli.js` entry with the subcommand, the
/// package selection, the option boundary, and the selected bin each in
/// its proper position. Three forms, and only these three:
/// - `npm exec --package=<omp> -- omp`: the `exec` subcommand first,
///   then the option grammar below;
/// - `npx --package=<omp> -- omp`: the same option grammar, with no
///   subcommand — npx takes options directly;
/// - `npx <omp>`: the bare positional package form, with no
///   launcher-level option trailing it.
///
/// The option grammar is one or more exact `--package` selectors —
/// each joined (`--package=<pkg>`) or separated (`--package <pkg>`),
/// repeats allowed — then the `--` boundary, then OMP's bin. At
/// least one selector is required: a bare `-- omp` selects nothing
/// and refuses.
///
/// Everything else refuses: command-string modes (`-c`, `--call`,
/// `--call=...`), short package selectors, unknown options, flags
/// before the subcommand, the `x` alias, version-pinned selections, a
/// missing boundary or command, and any foreign command. Tokens past
/// the selected command are the command's own arguments and are never
/// launcher evidence — and nothing past an unrecognized shape is
/// either. The Bun-resulting rule is enforced by the runtime below,
/// not here: this recognizes the launcher, and only the launcher.
fn is_omp_npm_launcher(argv: &[Vec<u8>]) -> bool {
    let [program, entry, rest @ ..] = argv else {
        return false;
    };
    if !is_node_image(program) {
        return false;
    }
    let is_npm = entry.ends_with(b"npm/bin/npm-cli.js");
    let is_npx = entry.ends_with(b"npm/bin/npx-cli.js");
    if !is_npm && !is_npx {
        return false;
    }
    // npx's bare positional package form: the FIRST token names the
    // package and npx runs its bin. No options precede it — anything
    // before the command position would make the token a command
    // spelling instead, which a package id never is.
    if is_npx
        && rest
            .first()
            .is_some_and(|token| token.as_slice() == OMP_PACKAGE)
    {
        return rest[1..]
            .iter()
            .all(|token| !is_npm_launcher_level_token(token));
    }
    // The option form: npm names its `exec` subcommand first — flags
    // before the subcommand are not an observed shape — while npx
    // takes options directly. Past this point the two parse
    // identically: exact `--package` selections, one `--` boundary,
    // then OMP's bin.
    let options = if is_npm {
        let [subcommand, options @ ..] = rest else {
            return false;
        };
        if subcommand != b"exec" {
            return false;
        }
        options
    } else {
        rest
    };
    let mut selected = false;
    let mut index = 0;
    while index < options.len() {
        let token = &options[index];
        if token == b"--" {
            // The option boundary: the next token is the selected
            // command, and it must be OMP's own bin. Everything past
            // it is the command's arguments — never launcher evidence.
            return selected
                && options
                    .get(index + 1)
                    .is_some_and(|command| command.as_slice() == OMP_BIN);
        }
        if token == b"--package" {
            let Some(value) = options.get(index + 1) else {
                return false;
            };
            if value != OMP_PACKAGE {
                return false;
            }
            selected = true;
            index += 2;
            continue;
        }
        if let Some(value) = token.strip_prefix(b"--package=") {
            // Exact selection only: a version pin or any other package
            // is not this launch's OMP, and refuses rather than
            // selecting.
            if value != OMP_PACKAGE {
                return false;
            }
            selected = true;
            index += 1;
            continue;
        }
        // Anything else before the boundary — a command string in any
        // spelling, a short selector, an unknown option, or a bare
        // command without its boundary — is not the supported form.
        return false;
    }
    // Options exhausted with no boundary and no command: a bare
    // `--package` selection names no bin to run.
    false
}

/// Whether a shell link is the known transparent trampoline for the
/// expected runtime program: `sh -c 'exec <runtime> ...'` as exactly one
/// simple command. The target program must be the launch's own
/// runtime/launcher binary — a trampoline exec'ing anything else is not
/// this launch's transparency. An `exec` that leaves no link (the usual
/// case) needs no rule at all.
fn is_omp_shell_trampoline(exe: &[u8], argv: &[Vec<u8>], expected: &[&[u8]]) -> bool {
    let name = omp_image_basename(exe);
    if !matches!(name, b"sh" | b"bash" | b"dash") {
        return false;
    }
    let [_, flag, command] = argv else {
        return false;
    };
    if flag != b"-c" {
        return false;
    }
    let command = match std::str::from_utf8(command) {
        Ok(command) => command,
        Err(_) => return false,
    };
    if has_unquoted_shell_syntax(command) {
        return false;
    }
    match shell_words::split(command) {
        Ok(words) => match words.as_slice() {
            [exec, target, ..] if exec == "exec" => expected.contains(&target.as_bytes()),
            _ => false,
        },
        Err(_) => false,
    }
}

/// OMP's instance of the restrictive corridor, over an already-walked
/// chain: the reporter (first link, never itself a runtime candidate) must
/// be the supported hook invocation; exactly one link must be the launched
/// runtime's descriptor (Bun plus the selected entry, or the compiled
/// target with TUI grammar); the pane anchor (last link) is accepted by
/// position; links below the runtime must be narrow hook trampolines, and
/// links above it must be recognized launchers or transparent shell
/// trampolines for the launch's own program. Any additional
/// session-hosting runtime of any kind and every unclassified intermediary
/// refuses.
///
/// `program` is the durable launch's program classification: it selects
/// which launcher spellings may appear above the runtime, so a chain whose
/// live shape contradicts its launch refuses rather than being re-explained.
///
/// The launcher predicate plus the trampoline target spellings one OMP
/// launch program admits above its runtime.
type OmpLauncherRules<'a> = (fn(&[Vec<u8>]) -> bool, &'a [&'a [u8]]);

/// OMP's instance of the restrictive corridor, over an already-walked
/// chain: the reporter (first link, never itself a runtime candidate) must
/// be the supported hook invocation; exactly one link must be the launched
/// runtime's descriptor (Bun plus the selected entry, or the compiled
/// target with TUI grammar); the pane anchor (last link) is accepted by
/// position; links below the runtime must be narrow hook trampolines, and
/// links above it must be recognized launchers or transparent shell
/// trampolines for the launch's own program. Any additional
/// session-hosting runtime of any kind and every unclassified intermediary
/// refuses.
///
/// `program` is the durable launch's program classification: it selects
/// which launcher spellings may appear above the runtime, so a chain whose
/// live shape contradicts its launch refuses rather than being re-explained.
///
/// Pure over the chain so the shapes are unit-testable without live
/// processes; [`foreground_omp_emitter`] supplies the walked chain.
fn omp_corridor(
    chain: &[ChainLink],
    program: &crate::agent_kind::omp::OmpLaunchProgram,
) -> Result<ProcessIdentity, String> {
    use crate::agent_kind::omp::OmpLaunchProgram;
    let reporter = chain
        .first()
        .ok_or_else(|| "the hook ancestry is empty".to_string())?;
    match reporter.argv.as_deref() {
        Some(argv) if is_hook_invocation_argv(argv) => {}
        _ => {
            return Err("the reporting process is not the supported hook invocation".to_string());
        }
    }
    // Which launcher spellings and trampoline targets this launch program
    // admits above its runtime. Direct Node execution and unknown programs
    // refuse before any link is examined.
    let (launchers, trampoline_targets): OmpLauncherRules<'_> = match program {
        OmpLaunchProgram::Omp => (no_omp_launcher, &[b"omp".as_slice()]),
        OmpLaunchProgram::Bun => (
            is_omp_bun_launcher,
            &[b"bun".as_slice(), b"bunx".as_slice()],
        ),
        OmpLaunchProgram::Npm => (
            is_omp_npm_launcher,
            &[b"npm".as_slice(), b"npx".as_slice(), b"node".as_slice()],
        ),
        OmpLaunchProgram::Node => {
            return Err("Node-executed OMP is not a supported runtime".to_string());
        }
        OmpLaunchProgram::Shell | OmpLaunchProgram::Unknown => {
            return Err("the OMP launch shape is not a supported runtime".to_string());
        }
    };
    // The emitter search excludes the reporter: the hook invocation is the
    // reporter by definition, never the session-hosting runtime, even when
    // its image name would otherwise match.
    let mut emitter = None;
    let mut emitter_index = 0;
    for (index, link) in chain.iter().enumerate().skip(1) {
        if is_omp_runtime_link(link) {
            if emitter.is_some() {
                return Err("a nested OMP process cannot report for the foreground".to_string());
            }
            emitter = Some(ProcessIdentity {
                pid: link.pid,
                start: link.start,
            });
            emitter_index = index;
        }
    }
    let Some(emitter) = emitter else {
        // Name the unverified execution shape when it is the reason: a
        // Node-executed entry point is deliberately unsupported, not
        // merely unrecognized.
        if chain.iter().skip(1).any(|link| is_node_image(&link.exe)) {
            return Err("Node-executed OMP is not a supported runtime".to_string());
        }
        return Err("the hook has no attributable OMP runtime".to_string());
    };
    // The live runtime argv must still describe an interactive TUI
    // conversation: the launch passed this grammar at spawn, but a process
    // that exec'd into a utility or print shape afterwards is no longer
    // the interactive runtime the proof admitted.
    omp_runtime_tui_grammar(&chain[emitter_index])?;
    for (index, link) in chain.iter().enumerate() {
        if index == 0 || index == emitter_index || index + 1 == chain.len() {
            continue;
        }
        let below_runtime = index < emitter_index;
        if below_runtime {
            let trampoline = link
                .argv
                .as_deref()
                .is_some_and(|argv| is_hook_trampoline(&link.exe, argv));
            if trampoline {
                continue;
            }
        } else {
            let recognized = link.argv.as_deref().is_some_and(|argv| {
                launchers(argv) || is_omp_shell_trampoline(&link.exe, argv, trampoline_targets)
            });
            if recognized {
                continue;
            }
        }
        if is_other_session_runtime(&link.exe) {
            return Err(
                "another session-hosting runtime sits between the reporter and the foreground"
                    .to_string(),
            );
        }
        // A Node interpreter off the emitter is "Node-executed OMP" only
        // when its argv shows it running the OMP entry: npm tooling above
        // the runtime is node too, and labeling that "Node-executed" would
        // send debugging after the wrong shape. Anything else
        // unrecognized stays a generic intermediary.
        if is_node_image(&link.exe)
            && link.argv.as_deref().is_some_and(|argv| {
                argv.len() >= 2
                    && (is_omp_bundle_entry(&argv[1])
                        || omp_resolved_entry_matches(link.pid, &argv[1]))
            })
        {
            return Err("Node-executed OMP is not a supported runtime".to_string());
        }
        return Err(
            "an unclassified intermediary sits between the reporter and the foreground".to_string(),
        );
    }
    Ok(emitter)
}

/// A launcher matcher that admits nothing: launches of the installed `omp`
/// command run the runtime directly under the pane, so any launcher-shaped
/// link above the emitter contradicts the launch.
fn no_omp_launcher(_argv: &[Vec<u8>]) -> bool {
    false
}

/// Whether one non-reporter link is an OMP runtime image: Bun (the entry
/// check needs its argv, so an argv-less Bun link is not a candidate here
/// and refuses downstream as unclassified) or the compiled target.
///
/// The entry may be named through a symlink: the installed `omp` command
/// IS one, and the kernel hands Bun the launched spelling — observed live
/// as `bun <home>/.bun/bin/omp ...` — rather than the resolved
/// bundle path. The raw suffix match runs first; when it misses, the
/// spelling is resolved (against the process's own working directory for
/// a relative spelling) and the canonical path is matched instead. An
/// unresolvable spelling falls back to the raw bytes, which then refuse.
/// Resolution corroborates which code the interpreter was handed; it
/// does not identify anything by itself, and a same-user mimic of the
/// shape stays outside the proof's non-adversarial boundary.
fn is_omp_runtime_link(link: &ChainLink) -> bool {
    if is_compiled_omp_image(&link.exe) {
        return true;
    }
    if !is_bun_image(&link.exe) {
        return false;
    }
    link.argv.as_deref().is_some_and(|argv| {
        argv.len() >= 2
            && omp_image_basename(&argv[0]) == b"bun"
            && (is_omp_bundle_entry(&argv[1]) || omp_resolved_entry_matches(link.pid, &argv[1]))
    })
}

/// Whether the canonical path behind one entry spelling is the selected
/// OMP bundle or source tree. `None` on any failure — undecodable bytes,
/// an unreadable working directory, a dangling link — so callers treat
/// unresolvable spellings as the raw bytes they already refused.
fn omp_resolved_entry_matches(pid: u32, entry: &[u8]) -> bool {
    resolved_omp_entry(pid, entry).is_some_and(|resolved| is_omp_bundle_entry(&resolved))
}

/// The canonical path behind one entry spelling, or `None` when it
/// cannot be established. Blocking filesystem I/O: callers run on the
/// attribution thread, never the report path's async context.
fn resolved_omp_entry(pid: u32, entry: &[u8]) -> Option<Vec<u8>> {
    let entry = std::str::from_utf8(entry).ok()?;
    let path = std::path::Path::new(entry);
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        // A relative spelling resolves against the SUBJECT's working
        // directory, read from the kernel — never this process's.
        let cwd = std::fs::read_link(format!("/proc/{pid}/cwd")).ok()?;
        cwd.join(path)
    };
    std::fs::canonicalize(absolute)
        .ok()
        .map(|canonical| os_bytes(&canonical))
}

/// The raw bytes behind one path, for byte-level matching over kernel
/// evidence. Unix-only, like every other reader in this module.
fn os_bytes(path: &std::path::Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().to_vec()
}

/// Whether the emitter link's live argv still describes an interactive TUI
/// conversation, read through the same grammar injection uses: Bun-executed
/// entries contribute everything after interpreter and entry point, and the
/// compiled target everything after its program. Missing or undecodable
/// argv is missing evidence and refuses — an interpreted runtime without
/// its entry point is not attributable — as does any non-interactive or
/// exiting shape.
fn omp_runtime_tui_grammar(link: &ChainLink) -> Result<(), String> {
    let argv = link.argv.as_deref().ok_or_else(|| {
        "the OMP runtime's command line could not be read; the report is refused".to_string()
    })?;
    let omp_args = if is_compiled_omp_image(&link.exe) {
        argv.get(1..).unwrap_or_default()
    } else {
        argv.get(2..).unwrap_or_default()
    };
    let decoded: Option<Vec<String>> = omp_args
        .iter()
        .map(|arg| std::str::from_utf8(arg).map(str::to_string).ok())
        .collect();
    let Some(decoded) = decoded else {
        return Err(
            "the OMP runtime's command line is not valid UTF-8; the report is refused".to_string(),
        );
    };
    match crate::agent_kind::omp::omp_tui_args_decision(&decoded) {
        crate::agent_kind::omp::OmpInjection::Inject { .. } => Ok(()),
        crate::agent_kind::omp::OmpInjection::Leave(reason) => Err(format!(
            "the live OMP runtime no longer describes an interactive conversation: {reason}"
        )),
    }
}

/// Attribute a hook connection to the one OMP runtime under the owned pane
/// that the launch installed.
///
/// The walk is the shared [`walk_to_pane`] mechanics; the corridor applied
/// below is OMP's instance of the restrictive rule. `program` is the
/// durable launch's program classification, so the live chain must agree
/// with how the session was launched, not merely look like some OMP shape.
pub(crate) fn foreground_omp_emitter(
    peer: ProcessIdentity,
    pane_pid: u32,
    program: &crate::agent_kind::omp::OmpLaunchProgram,
) -> Result<ProcessIdentity, String> {
    let chain = walk_to_pane(peer, pane_pid)?;
    omp_corridor(&chain, program)
}

/// Whether one argv is exactly the Goose helper invocation:
/// `internal goose-hook` after any `argv[0]`. A Goose-specific rule
/// beside [`is_hook_invocation_argv`], which matches only `internal
/// hook`.
///
/// Exact, not a prefix: the helper takes no arguments, so a live
/// peer with trailing words is not the helper invocation this
/// contract describes. A future helper flag requires a corridor
/// update in the same change.
fn is_goose_reporter_argv(argv: &[Vec<u8>]) -> bool {
    argv.len() == 3 && argv[1] == b"internal" && argv[2] == b"goose-hook"
}

/// Whether one link's image is a native `goose` executable. A script
/// named `goose` never matches: scripts resolve to their
/// interpreter's image before this comparison runs. The ` (deleted)`
/// suffix a replaced-while-running binary carries is stripped before
/// comparing, the way the OMP half of this module does.
fn is_goose_runtime_link(link: &ChainLink) -> bool {
    omp_image_basename(&link.exe) == b"goose"
}

/// The exact persisted MCP reporter declaration's post-`sh -c` words:
/// `exec "${FARHELM_GOOSE_REPORTER_EXE:-farhelm}" internal goose-hook`.
/// A below-runtime link is the narrow MCP trampoline ONLY when it is
/// a shell whose `-c` command shell-splits to EXACTLY these words —
/// resolved exes, extra words, or different words are not this
/// launch's transparency, and `$`-leading expansions are matched ONLY
/// as these literal bytes (the split never expands).
fn is_goose_mcp_trampoline(exe: &[u8], argv: &[Vec<u8>]) -> bool {
    let name = exe.rsplit(|byte| *byte == b'/').next().unwrap_or_default();
    if !matches!(name, b"sh" | b"bash" | b"dash") {
        return false;
    }
    let [_, flag, command] = argv else {
        return false;
    };
    if flag != b"-c" {
        return false;
    }
    let command = match std::str::from_utf8(command) {
        Ok(command) => command,
        Err(_) => return false,
    };
    shell_words::split(command).is_ok_and(|words| {
        words.as_slice()
            == [
                "exec",
                "${FARHELM_GOOSE_REPORTER_EXE:-farhelm}",
                "internal",
                "goose-hook",
            ]
    })
}

/// Whether one above-runtime link is this launch's transparent shell:
/// a shell whose `-c` command shell-splits to exactly `exec goose`
/// plus arguments. Same rule OMP's `S` applies, no new generality —
/// a trampoline exec'ing anything else is not this launch's
/// transparency, and (like every shell rule here) has no business
/// with unquoted shell syntax anywhere past the words.
fn is_goose_shell_trampoline(link: &ChainLink) -> bool {
    let name = omp_image_basename(&link.exe);
    if !matches!(name, b"sh" | b"bash" | b"dash") {
        return false;
    }
    let Some(argv) = link.argv.as_deref() else {
        return false;
    };
    let [_, flag, command] = argv else {
        return false;
    };
    if flag != b"-c" {
        return false;
    }
    let command = match std::str::from_utf8(command) {
        Ok(command) => command,
        Err(_) => return false,
    };
    let words = match shell_words::split(command) {
        Ok(words) => words,
        Err(_) => return false,
    };
    let [exec, target, ..] = words.as_slice() else {
        return false;
    };
    exec == "exec" && *target == "goose" && !has_unquoted_shell_syntax(command)
}

/// Whether the emitter link's live argv still describes a Goose
/// session, read through the SAME grammar injection uses — decoded
/// to UTF-8 here, decided there. Missing or undecodable argv is
/// missing evidence and refuses, as does any shape the grammar does
/// not recognize: the corridor needs "recognized" only (fresh,
/// resume, fork, and edit launches all emit reports worth
/// attributing), but a utility argv can never carry the runtime.
fn goose_runtime_session_grammar(link: &ChainLink) -> Result<(), String> {
    let argv = link.argv.as_deref().ok_or_else(|| {
        "the Goose runtime's command line could not be read; the report is refused".to_string()
    })?;
    let decoded: Option<Vec<String>> = argv
        .iter()
        .map(|arg| std::str::from_utf8(arg).map(str::to_string).ok())
        .collect();
    let Some(decoded) = decoded else {
        return Err(
            "the Goose runtime's command line is not valid UTF-8; the report is refused"
                .to_string(),
        );
    };
    match crate::agent_kind::goose::goose_launch_shape(&decoded) {
        Some(_) => Ok(()),
        None => Err(
            "the live Goose runtime no longer describes a session; the report is refused"
                .to_string(),
        ),
    }
}

/// Goose's instance of the restrictive corridor, over an
/// already-walked chain: the reporter (first link, the
/// kernel-attributed socket peer) must be exactly the helper
/// invocation; exactly one native `goose` image is the emitter, whose
/// LIVE argv must still describe a session through the shared
/// grammar; the pane anchor (last link, the owned foreground) is
/// accepted by position; below the runtime only the narrow MCP
/// trampoline may appear; above it only this launch's transparent
/// shell. Any second `goose`, any other session-hosting runtime, or
/// any unclassified intermediary refuses.
///
/// Pure over the chain so the shapes are unit-testable without live
/// processes; every refusal names the shape it found.
fn goose_corridor(
    chain: &[ChainLink],
    program: &crate::agent_kind::goose::GooseLaunchProgram,
) -> Result<ProcessIdentity, String> {
    use crate::agent_kind::goose::GooseLaunchProgram;
    if !matches!(
        program,
        GooseLaunchProgram::Goose | GooseLaunchProgram::Shell
    ) {
        return Err(
            "the Goose launch shape is not a supported runtime; the report is refused".to_string(),
        );
    }
    let Some(reporter) = chain.first() else {
        return Err("the process chain is empty; the report is refused".to_string());
    };
    let reporter_argv = reporter.argv.as_deref().ok_or_else(|| {
        "the reporter's command line could not be read; the report is refused".to_string()
    })?;
    if !is_goose_reporter_argv(reporter_argv) {
        return Err(
            "the reporter is not the Goose helper invocation; the report is refused".to_string(),
        );
    }
    let mut emitter_index = None;
    for (index, link) in chain.iter().enumerate().skip(1) {
        if is_goose_runtime_link(link) {
            if emitter_index.is_some() {
                return Err(
                    "two live Goose runtimes claim the report; the report is refused".to_string(),
                );
            }
            emitter_index = Some(index);
        }
    }
    let Some(emitter_index) = emitter_index else {
        return Err("no live Goose runtime claims the report; the report is refused".to_string());
    };
    for link in &chain[1..emitter_index] {
        let argv = link.argv.as_deref().ok_or_else(|| {
            "a process between the reporter and the Goose runtime has no command line; the report is refused"
                .to_string()
        })?;
        if !is_goose_mcp_trampoline(&link.exe, argv) {
            return Err(
                "a process between the reporter and the Goose runtime is not the MCP trampoline; the report is refused"
                    .to_string(),
            );
        }
    }
    // The runtime's OWN argv is re-read live and must still describe a
    // session: the launch-time classification says how this session
    // started, not what this process is now.
    goose_runtime_session_grammar(&chain[emitter_index])?;
    // Links strictly between the emitter and the pane anchor — empty
    // when the emitter IS the anchor (an exec'd pane), which the
    // bounds check keeps from slicing backwards.
    let last = chain.len() - 1;
    if emitter_index + 1 < last {
        for link in &chain[emitter_index + 1..last] {
            if is_goose_shell_trampoline(link) {
                continue;
            }
            // A second `goose` cannot reach this loop — the emitter
            // scan above refuses it first, with the nesting
            // diagnostic — so only the other session-hosting
            // runtimes are named here.
            if is_other_session_runtime(&link.exe) {
                return Err(
                    "another session-hosting runtime sits above the Goose emitter; the report is refused"
                        .to_string(),
                );
            }
            return Err(
                "an unclassified process sits above the Goose emitter; the report is refused"
                    .to_string(),
            );
        }
    }
    Ok(ProcessIdentity {
        pid: chain[emitter_index].pid,
        start: chain[emitter_index].start,
    })
}

/// Attribute a hook connection to the one Goose runtime under the owned pane
/// that the launch installed.
///
/// The walk is the shared [`walk_to_pane`] mechanics; the corridor applied
/// below is Goose's instance of the restrictive rule. `program` is the
/// durable launch's program classification, so the live chain must agree
/// with how the session was launched, not merely look like some Goose shape.
pub(crate) fn foreground_goose_emitter(
    peer: ProcessIdentity,
    pane_pid: u32,
    program: &crate::agent_kind::goose::GooseLaunchProgram,
) -> Result<ProcessIdentity, String> {
    let chain = walk_to_pane(peer, pane_pid)?;
    goose_corridor(&chain, program)
}

/// Extract the environment region of a macOS `KERN_PROCARGS2` buffer,
/// re-joined as the NUL-delimited block [`read_environ`] promises.
///
/// # Why this excludes argv, and why that is load-bearing
///
/// `KERN_PROCARGS2` hands back one blob holding the exec path, the full
/// argv, AND the environment; `/proc/<pid>/environ` on Linux holds the
/// environment alone. The platforms MUST agree that only what a process
/// was exec'd with in its envp can claim it for a kill sweep, because argv
/// is trivially attacker- and accident-controlled from outside: a user
/// running `grep FARHELM_SESSION_ID=<id> ...`, or an editor opening a file
/// whose name contains the marker text, would otherwise be swept up and
/// SIGKILLed by that session's stop. So the argv region is parsed only to
/// be skipped over.
///
/// # The layout, and what this tolerates
///
/// `[argc: i32 native-endian][exec path\0][\0 padding][argv[0]\0 ...
/// argv[argc-1]\0][env\0 ...]`. The padding after the exec path is
/// alignment slack the kernel inserts, so it is skipped before argv is
/// counted — miscounting there would slide the argv/env boundary and let
/// command-line bytes into the result, which is the one outcome this
/// function must never produce. The environment region ends at the first
/// EMPTY entry or at the end of the buffer; a trailing entry with no
/// terminating NUL (a truncated read) is dropped rather than guessed at.
///
/// The tail of that region may also contain the kernel's "apple" strings
/// (`executable_path=`, `ptr_munge=`, `stack_guard=`, ...), which are not
/// environment variables but are shaped like them. They are harmless
/// passengers: marker matching is by complete, exact NUL-delimited entry,
/// and none of those keys is a farhelm marker.
///
/// `None` for a structurally unusable buffer (too short to hold `argc`, no
/// terminated exec path). Callers treat that exactly like an unreadable
/// environment.
///
/// Compiled on every platform, not just macOS, so its tests run in ordinary
/// CI on Linux: this is pure byte parsing with no syscall behind it, and it
/// is the part of the macOS path most worth pinning. The `test` arm of the
/// cfg is what keeps it from being dead code in a non-mac release build.
#[cfg(any(target_os = "macos", test))]
fn parse_procargs2(buf: &[u8]) -> Option<Vec<u8>> {
    let argc_bytes: [u8; 4] = buf.get(..4)?.try_into().ok()?;
    let argc = i32::from_ne_bytes(argc_bytes).max(0) as usize;

    // The exec path, then whatever alignment padding follows it. The
    // padding skip is unconditional rather than bounded by a count: only
    // one such run exists in the buffer (right here, before argv[0]), and
    // an empty argv[0] — the one thing this could swallow by mistake — is
    // not a shape any exec in this tree produces.
    let mut rest = buf.get(4..)?;
    let path_end = rest.iter().position(|&b| b == 0)?;
    rest = &rest[path_end + 1..];
    let argv_start = rest.iter().position(|&b| b != 0).unwrap_or(rest.len());
    rest = &rest[argv_start..];

    // Step over exactly argc NUL-terminated strings. Running out of buffer
    // mid-argv means there is no environment region at all, which is an
    // empty environment rather than a parse failure.
    for _ in 0..argc {
        match rest.iter().position(|&b| b == 0) {
            Some(end) => rest = &rest[end + 1..],
            None => return Some(Vec::new()),
        }
    }

    let mut environ = Vec::new();
    while let Some(end) = rest.iter().position(|&b| b == 0) {
        if end == 0 {
            // An empty entry terminates the environment region.
            break;
        }
        environ.extend_from_slice(&rest[..end]);
        environ.push(0);
        rest = &rest[end + 1..];
    }
    Some(environ)
}

/// Extract the argv region of a macOS `KERN_PROCARGS2` buffer: exactly
/// `argc` NUL-terminated strings after the exec path and its alignment
/// padding.
///
/// The header skip mirrors [`parse_procargs2`] — same buffer, same
/// layout, only a different region — but the failure direction is the
/// opposite: running out of buffer mid-argv is `None`, never an empty
/// argv. An empty argv would be a prefix a later proof could mistake for
/// the whole command line ("the process was invoked bare"), while the
/// truth is that the observation was cut short; refusing the evidence
/// keeps a truncated read from becoming a false entry-point match. The
/// byte total is capped at [`MAX_ARGV_BYTES_PER_PROCESS`] for the same
/// reason the fetch buffer is sized to it.
///
/// Like its sibling, the `test` arm of the cfg keeps it (and its tests)
/// alive in ordinary Linux CI.
#[cfg(any(target_os = "macos", test))]
fn parse_procargs2_argv(buf: &[u8]) -> Option<Vec<Vec<u8>>> {
    let argc_bytes: [u8; 4] = buf.get(..4)?.try_into().ok()?;
    let argc = i32::from_ne_bytes(argc_bytes).max(0) as usize;
    // A garbage argc must not size the collection: the loop below would
    // stop at the buffer end anyway, but the capacity reservation happens
    // first.
    if argc > 4096 {
        return None;
    }

    let mut rest = buf.get(4..)?;
    let path_end = rest.iter().position(|&b| b == 0)?;
    rest = &rest[path_end + 1..];
    let argv_start = rest.iter().position(|&b| b != 0).unwrap_or(rest.len());
    rest = &rest[argv_start..];

    let mut argv = Vec::with_capacity(argc.min(64));
    let mut total = 0usize;
    for _ in 0..argc {
        let end = rest.iter().position(|&b| b == 0)?;
        total += end;
        if total > MAX_ARGV_BYTES_PER_PROCESS {
            return None;
        }
        argv.push(rest[..end].to_vec());
        rest = &rest[end + 1..];
    }
    Some(argv)
}

/// Linux: the process table is `/proc`.
///
/// This is the original M2 implementation, moved here unchanged rather
/// than rewritten — including the `hidepid` handling and the last-`)`
/// `comm` parsing, both of which exist because of specific things real
/// hosts do.
#[cfg(not(target_os = "macos"))]
mod imp {
    use super::ProcessState;
    use std::collections::HashMap;

    /// Whether an I/O error reading some `/proc/<pid>/...` path means the
    /// process (or its row) is simply gone, as opposed to a genuine problem
    /// this sweep must report. `ENOENT` is the ordinary shape (the path
    /// itself vanished), but `ESRCH` comes through this path too: opening a
    /// still-listed `/proc/<pid>/stat` whose process dies mid-read fails
    /// with ESRCH rather than ENOENT (observed on CI — the confirmation poll
    /// raced a SIGKILL'd pid's teardown and reported a false sweep failure),
    /// so both mean the same thing here: nothing left to worry about.
    fn is_gone_errno(e: &std::io::Error) -> bool {
        e.kind() == std::io::ErrorKind::NotFound || e.raw_os_error() == Some(libc::ESRCH)
    }

    /// Read one stat field by its position AFTER the `comm` field (index 0 is
    /// `state`, the third stat field overall). `rest` is everything following
    /// the LAST `)` in a `/proc/<pid>/stat` line — see [`parse_stat`] for why
    /// that boundary is found on raw bytes before this ever touches `&str`.
    fn stat_field(rest: &str, index: usize) -> Option<&str> {
        rest.split_whitespace().nth(index)
    }

    /// Parse a `/proc/<pid>/stat` line's state, parent pid, and kernel
    /// start-time (state is field 3 overall, the first token after `comm`;
    /// start-time is field 22, the 20th token after `comm`) from raw bytes.
    ///
    /// Bytes in, not `&str`, and the `comm` field's own bytes are never
    /// decoded at all: `comm` (the process name in parentheses) is whatever
    /// bytes the process named itself with via `PR_SET_NAME`/`argv[0]` and can
    /// contain arbitrary, non-UTF-8 data, spaces, or even parentheses — so
    /// this locates the LAST `)` as a raw byte search (valid regardless of
    /// what came before it) and only decodes the fixed-format, always-ASCII
    /// fields after it. A `comm` with a non-UTF-8 byte would otherwise fail
    /// the whole read, silently misreporting a live process as "gone" to
    /// [`super::snapshot`]'s caller — exactly the kind of
    /// resource-exhaustion-disguised-as-success bug the sweep works hard
    /// elsewhere to avoid.
    ///
    /// State is what lets the sweep's confirmation step recognize a zombie —
    /// a process that has already exited but has no ancestor left to reap it
    /// — as gone rather than as a stuck SIGKILL. Start-time is what makes a
    /// discovered pid safe to act on LATER, after other work (a signal, a
    /// sleep, another `/proc` walk) has given the kernel a chance to reuse
    /// it: `sweep::signal_validated` re-reads this same field immediately
    /// before signaling and refuses to act unless it still matches, which is
    /// the only way a numeric pid recorded minutes, seconds, or even
    /// microseconds ago can still be trusted.
    fn parse_stat(bytes: &[u8]) -> Result<(u32, u64, char), String> {
        let Some(after_comm) = bytes.iter().rposition(|&b| b == b')') else {
            return Err(format!(
                "stat content has no ')' delimiting comm: {bytes:?}"
            ));
        };
        let rest = std::str::from_utf8(&bytes[after_comm + 1..])
            .map_err(|e| format!("stat fields after comm are not valid UTF-8: {e}"))?;
        let state = stat_field(rest, 0)
            .ok_or("stat content is missing the state field")?
            .chars()
            .next()
            .ok_or("stat content has an empty state field")?;
        let ppid = stat_field(rest, 1)
            .ok_or("stat content is missing the ppid field")?
            .parse::<u32>()
            .map_err(|e| format!("stat ppid field is unparseable: {e}"))?;
        let starttime = stat_field(rest, 19)
            .ok_or("stat content is missing the starttime field")?
            .parse::<u64>()
            .map_err(|e| format!("stat starttime field is unparseable: {e}"))?;
        Ok((ppid, starttime, state))
    }

    /// This process's own effective uid, for [`is_own_pid_dir`].
    fn euid() -> u32 {
        // SAFETY: geteuid takes no arguments and cannot fail.
        unsafe { libc::geteuid() }
    }

    /// Whether `/proc/<pid>` is owned by this process's own effective uid.
    ///
    /// Exists for hosts whose `/proc` is mounted `hidepid=1` (or stricter): a
    /// legitimate, common hardening option under which OTHER users' pid
    /// directories stay visible to `readdir` — so the walk below still
    /// enumerates them — but their contents (`stat`, `environ`, ...)
    /// become `EACCES`. That is routine and expected, not a sweep failure,
    /// so the host-wide snapshot calls this BEFORE reading any row. The
    /// single-pid reader calls it only AFTER a row read fails: a readable row
    /// still carries the start-time identity that callers should compare,
    /// while a foreign replacement hidden by `hidepid` is an ordinary miss
    /// rather than a teardown error. A pid that has already exited (or
    /// otherwise can't be stat'd at the directory level) is not this check's
    /// business to adjudicate — [`read_stat`]'s own `ENOENT` handling covers
    /// that — so failure to read the directory's metadata defaults to "ours",
    /// leaving the decision to the caller's normal fail-closed path.
    fn is_own_pid_dir(pid: u32) -> bool {
        use std::os::unix::fs::MetadataExt;
        match std::fs::metadata(format!("/proc/{pid}")) {
            Ok(metadata) => metadata.uid() == euid(),
            Err(_) => true,
        }
    }

    /// Read and parse `/proc/<pid>/stat`, keeping the raw state character
    /// so [`snapshot`] can ignore it and [`read_process`] can classify it.
    ///
    /// `Ok(None)` means the process is simply gone (`ENOENT`/`ESRCH`).
    /// Anything else that goes wrong — a permission error, a malformed or
    /// unrecognized stat format — comes back as `Err`. A caller may suppress
    /// that failure only after [`is_own_pid_dir`] proves the pid now belongs
    /// to another user; otherwise treating it as absence would let the sweep
    /// silently under-collect a live descendant and report itself clean when
    /// it was not.
    fn read_stat(pid: u32) -> Result<Option<(u32, u64, char)>, String> {
        match std::fs::read(format!("/proc/{pid}/stat")) {
            Ok(bytes) => parse_stat(&bytes).map(Some),
            Err(e) if is_gone_errno(&e) => Ok(None),
            Err(e) => Err(format!("reading /proc/{pid}/stat: {e}")),
        }
    }

    /// See [`super::snapshot`]. `readdir` over `/proc`, one numeric entry
    /// at a time; foreign-uid pids are skipped before either read can turn
    /// an ordinary permission restriction into a reported failure.
    pub(super) fn snapshot() -> Result<(super::ProcessTable, Vec<String>), String> {
        let mut stats = HashMap::new();
        let mut soft_errors = Vec::new();
        let entries = std::fs::read_dir("/proc").map_err(|e| format!("reading /proc: {e}"))?;
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    soft_errors.push(format!("iterating /proc: {e}"));
                    continue;
                }
            };
            let Some(pid) = entry
                .file_name()
                .to_str()
                .and_then(|s| s.parse::<u32>().ok())
            else {
                continue;
            };
            if !is_own_pid_dir(pid) {
                continue;
            }
            match read_stat(pid) {
                Ok(Some((ppid, starttime, _state))) => {
                    stats.insert(pid, (ppid, starttime));
                }
                Ok(None) => {}
                Err(e) => soft_errors.push(e),
            }
        }
        Ok((stats, soft_errors))
    }

    /// See [`super::read_process`]. When a row read fails, re-check its uid to
    /// distinguish a genuine failure for an owned process from a pid that the
    /// kernel recycled for another user. The latter is unreadable on a
    /// `hidepid=1` procfs mount, but it is no longer a process this sweep is
    /// responsible for reporting or signaling. Successful reads retain their
    /// full identity for the caller's start-time comparison, even if the
    /// process changed credentials after the earlier snapshot.
    ///
    /// `Z` is the kernel's zombie state letter; every other state means the
    /// process can still run.
    pub(super) fn read_process(pid: u32) -> Result<Option<(u32, u64, ProcessState)>, String> {
        let stat = match read_stat(pid) {
            Ok(stat) => stat,
            Err(_) if !is_own_pid_dir(pid) => return Ok(None),
            Err(error) => return Err(error),
        };
        Ok(stat.map(|(ppid, starttime, state)| {
            let state = if state == 'Z' {
                ProcessState::Zombie
            } else {
                ProcessState::Running
            };
            (ppid, starttime, state)
        }))
    }

    /// See [`super::read_environ`]. `/proc/<pid>/environ` is already
    /// exactly the NUL-delimited block the contract describes, so there is
    /// nothing to parse: every failure (gone, foreign uid, non-dumpable)
    /// collapses to `None`.
    pub(super) fn read_environ(pid: u32) -> Option<Vec<u8>> {
        std::fs::read(format!("/proc/{pid}/environ")).ok()
    }

    /// The raw bytes of `/proc/<pid>/exe`: the same source the emitter
    /// check has always read, exposed so the shared walk can capture the
    /// image once per edge instead of every consumer re-reading it.
    /// Failures (gone, foreign uid, non-dumpable) refuse the walk, as
    /// they always refused the report.
    pub(super) fn process_exe(pid: u32) -> Result<Vec<u8>, String> {
        use std::os::unix::ffi::OsStrExt;
        std::fs::read_link(format!("/proc/{pid}/exe"))
            .map(|path| path.as_os_str().as_bytes().to_vec())
            .map_err(|error| format!("reading process {pid} executable: {error}"))
    }

    /// Whether raw `exe` bytes name a native Codex image: the basename
    /// check the emitter has always applied, over bytes the caller
    /// already captured, so the walk and the corridor cannot observe two
    /// different images for one edge.
    pub(super) fn is_codex_image(exe: &[u8]) -> bool {
        // Byte-level basename, exactly as the emitter has always applied
        // it: no UTF-8 decoding, so a non-UTF-8 image path decides on its
        // raw bytes rather than on a lossy rendering of them.
        let name = exe.rsplit(|byte| *byte == b'/').next().unwrap_or_default();
        // procfs marks the image this way after an atomic executable update.
        name.strip_suffix(b" (deleted)").unwrap_or(name) == b"codex"
    }

    /// The bounded `/proc/<pid>/cmdline` split: NUL-separated argv with
    /// the trailing NUL the kernel always writes consumed, capped at
    /// [`super::MAX_ARGV_BYTES_PER_PROCESS`]. Over-budget, truncated
    /// (no terminating NUL), or unreadable reads are `None` — missing
    /// evidence, not a walk failure.
    pub(super) fn read_process_argv(pid: u32) -> Option<Vec<Vec<u8>>> {
        use std::io::Read;
        let mut limited = std::fs::File::open(format!("/proc/{pid}/cmdline"))
            .ok()?
            .take((super::MAX_ARGV_BYTES_PER_PROCESS + 1) as u64);
        let mut buf = Vec::new();
        limited.read_to_end(&mut buf).ok()?;
        if buf.len() > super::MAX_ARGV_BYTES_PER_PROCESS || !buf.ends_with(b"\0") {
            return None;
        }
        buf.pop();
        if buf.is_empty() {
            return Some(Vec::new());
        }
        Some(
            buf.split(|byte| *byte == 0)
                .map(|arg| arg.to_vec())
                .collect(),
        )
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// `parse_stat`'s whole reason to exist: a `comm` field containing
        /// spaces AND a stray closing paren must not fool the last-`)` search
        /// into stopping early. This pins the kernel's actual escape hatch —
        /// `comm` can contain anything, including `)`, so only the LAST `)`
        /// in the whole line is the real delimiter, no matter how many
        /// look-alikes precede it.
        #[farhelm_testtrace::test]
        fn parse_stat_handles_comm_with_parens_and_spaces() {
            // comm = "1 (weird) name)" — spaces, an internal paren pair, AND
            // a trailing stray ')' that is NOT the kernel's own delimiter.
            // Wrapped by the kernel in its own parens, the line's tail reads
            // "...name))" — two closing parens back to back — and only the
            // second is real.
            let line: &[u8] =
                b"123 (1 (weird) name)) S 456 1 1 0 -1 0 0 0 0 0 0 0 0 0 20 0 1 0 789";
            let (ppid, starttime, state) =
                parse_stat(line).expect("well-formed synthetic stat line");
            assert_eq!(ppid, 456, "ppid must be read from AFTER the true delimiter");
            assert_eq!(starttime, 789, "starttime is the 20th field after comm");
            assert_eq!(state, 'S', "state is the first field after comm");
        }

        /// `comm` is whatever bytes the process named itself with — it can be
        /// genuinely non-UTF-8 — and `parse_stat` must not choke on that, since
        /// only the LAST `)` is located via a raw byte search and everything
        /// before it (the non-UTF-8 comm included) is never decoded at all.
        /// Failing this would misreport a live, oddly-named process as
        /// unparseable, folding it into a reported sweep error over nothing
        /// more than a name it never chose to be `/proc`-friendly about.
        #[farhelm_testtrace::test]
        fn parse_stat_survives_non_utf8_bytes_in_comm() {
            let mut line = b"123 (bad".to_vec();
            line.push(0xff); // not valid UTF-8 on its own or in context here
            line.extend_from_slice(b"name) Z 456 1 1 0 -1 0 0 0 0 0 0 0 0 0 20 0 1 0 789");
            let (ppid, starttime, state) =
                parse_stat(&line).expect("a non-UTF-8 comm must not fail parsing");
            assert_eq!(ppid, 456);
            assert_eq!(starttime, 789);
            assert_eq!(state, 'Z');
        }

        /// A stat line with no `)` at all (never happens for a real kernel-
        /// written row, but a corrupted read or a hostile fixture could
        /// produce one) must be a reported parse error, not a silent "gone" —
        /// conflating "malformed" with "absent" would let a genuinely live,
        /// misread process vanish from a sweep without a trace.
        #[farhelm_testtrace::test]
        fn parse_stat_rejects_a_line_with_no_delimiter() {
            assert!(parse_stat(b"garbage with no parens at all").is_err());
        }

        /// The zombie mapping is the one state distinction the sweep's
        /// confirmation step depends on, and it is asserted here against a
        /// REAL process rather than a fixture because the mapping is only
        /// meaningful if the live read produces it: this process is its own
        /// witness, and it is certainly not a zombie.
        #[farhelm_testtrace::test]
        fn a_live_process_reads_back_as_running() {
            let me = std::process::id();
            let (_, _, state) = read_process(me)
                .expect("reading this process's own stat must not error")
                .expect("this process certainly exists");
            assert_eq!(state, ProcessState::Running);
        }
    }
}

/// macOS: the process table is `sysctl`, since there is no `/proc`.
///
/// Two `sysctl` trees carry everything the sweep needs. `kern.proc`
/// (`KERN_PROC_ALL` / `KERN_PROC_PID`) yields `struct kinfo_proc` rows with
/// parent pid, effective uid, start time, and process state; `kern.procargs2`
/// yields one process's exec-time argv and environment. Both are readable
/// without privilege for one's OWN processes and refuse (or omit) other
/// users', which is the same same-euid scope the Linux walk enforces
/// explicitly.
///
/// # Why the layout is spelled out here
///
/// The `libc` crate defines `kinfo_proc` for the BSDs but not for Apple, so
/// the layout below is transcribed from `<sys/sysctl.h>` (`kinfo_proc`,
/// `eproc`, `_pcred`, `_ucred`) and `<sys/proc.h>` (`extern_proc`) — a
/// frozen, decades-old ABI that `ps` and every process lister on the
/// platform already depend on. Only four fields are ever read; every other
/// field exists solely to place those four at the right offsets, which is
/// why the whole thing is `#[allow(dead_code)]`. Because a silent layout
/// mistake here would misread parent pids and uids rather than fail loudly,
/// the size and all four read offsets are asserted at COMPILE time against
/// the values Apple's headers produce.
#[cfg(target_os = "macos")]
mod imp {
    use super::ProcessState;
    use std::collections::HashMap;
    use std::ffi::{c_char, c_int, c_short, c_uchar, c_uint, c_ushort, c_void};
    use std::mem::{offset_of, size_of};

    /// `struct extern_proc` from `<sys/proc.h>`: the "process" half of a
    /// `kinfo_proc` row. Only `p_starttime` and `p_stat` are read.
    ///
    /// The leading field is a union in C (`p_un`, holding either a
    /// run-queue pointer pair or the start time); both arms are 16 bytes,
    /// and only the start-time arm is meaningful in a `kinfo_proc` handed
    /// out by `sysctl`, so it is spelled as the `timeval` directly.
    #[repr(C)]
    #[allow(dead_code, non_snake_case)]
    struct ExternProc {
        p_starttime: libc::timeval,
        p_vmspace: *mut c_void,
        p_sigacts: *mut c_void,
        p_flag: c_int,
        p_stat: c_char,
        p_pid: libc::pid_t,
        p_oppid: libc::pid_t,
        p_dupfd: c_int,
        user_stack: *mut c_char,
        exit_thread: *mut c_void,
        p_debugger: c_int,
        sigwait: c_int,
        p_estcpu: c_uint,
        p_cpticks: c_int,
        p_pctcpu: u32,
        p_wchan: *mut c_void,
        p_wmesg: *mut c_char,
        p_swtime: c_uint,
        p_slptime: c_uint,
        p_realtimer: libc::itimerval,
        p_rtime: libc::timeval,
        p_uticks: u64,
        p_sticks: u64,
        p_iticks: u64,
        p_traceflag: c_int,
        p_tracep: *mut c_void,
        p_siglist: c_int,
        p_textvp: *mut c_void,
        p_holdcnt: c_int,
        p_sigmask: u32,
        p_sigignore: u32,
        p_sigcatch: u32,
        p_priority: c_uchar,
        p_usrpri: c_uchar,
        p_nice: c_char,
        p_comm: [c_char; 17],
        p_pgrp: *mut c_void,
        p_addr: *mut c_void,
        p_xstat: c_ushort,
        p_acflag: c_ushort,
        p_ru: *mut c_void,
    }

    /// `struct _pcred` from `<sys/sysctl.h>`. Never read; present to place
    /// the `_ucred` that follows it.
    #[repr(C)]
    #[allow(dead_code)]
    struct PCred {
        pc_lock: [c_char; 72],
        pc_ucred: *mut c_void,
        p_ruid: libc::uid_t,
        p_svuid: libc::uid_t,
        p_rgid: libc::gid_t,
        p_svgid: libc::gid_t,
        p_refcnt: c_int,
    }

    /// `struct _ucred` from `<sys/sysctl.h>`. `cr_uid` is the effective uid
    /// the same-euid filter compares against.
    #[repr(C)]
    #[allow(dead_code)]
    struct UCred {
        cr_ref: i32,
        cr_uid: libc::uid_t,
        cr_ngroups: c_short,
        cr_groups: [libc::gid_t; 16],
    }

    /// `struct vmspace` from `<sys/vm.h>`, in the shape `eproc` embeds it.
    /// Never read; present for its 64 bytes of offset.
    #[repr(C)]
    #[allow(dead_code)]
    struct VmSpace {
        vm_refcnt: c_int,
        vm_shm: *mut c_char,
        vm_rssize: i32,
        vm_swrss: i32,
        vm_tsize: i32,
        vm_dsize: i32,
        vm_ssize: i32,
        vm_taddr: *mut c_char,
        vm_daddr: *mut c_char,
        vm_maxsaddr: *mut c_char,
    }

    /// `struct eproc` from `<sys/sysctl.h>`: the "external" half of a
    /// `kinfo_proc` row. Only `e_ucred.cr_uid` and `e_ppid` are read.
    #[repr(C)]
    #[allow(dead_code)]
    struct EProc {
        e_paddr: *mut c_void,
        e_sess: *mut c_void,
        e_pcred: PCred,
        e_ucred: UCred,
        e_vm: VmSpace,
        e_ppid: libc::pid_t,
        e_pgid: libc::pid_t,
        e_jobc: c_short,
        e_tdev: i32,
        e_tpgid: libc::pid_t,
        e_tsess: *mut c_void,
        e_wmesg: [c_char; 8],
        e_xsize: i32,
        e_xrssize: c_short,
        e_xccount: c_short,
        e_xswrss: c_short,
        e_flag: i32,
        e_login: [c_char; 12],
        e_spare: [i32; 4],
    }

    /// `struct kinfo_proc` from `<sys/sysctl.h>` — one row of the
    /// `kern.proc` sysctl's answer.
    #[repr(C)]
    struct KinfoProc {
        kp_proc: ExternProc,
        kp_eproc: EProc,
    }

    /// The layout guard. These are the values Apple's own headers produce
    /// on 64-bit Darwin (verified by compiling `offsetof` assertions against
    /// them); a transcription slip anywhere above would move one of them and
    /// break the build, which is enormously preferable to reading a parent
    /// pid out of the middle of some other field at runtime and quietly
    /// sweeping the wrong tree.
    const _: () = {
        assert!(size_of::<KinfoProc>() == 648);
        assert!(offset_of!(KinfoProc, kp_proc.p_starttime) == 0);
        assert!(offset_of!(KinfoProc, kp_proc.p_stat) == 36);
        assert!(offset_of!(KinfoProc, kp_eproc.e_ucred.cr_uid) == 420);
        assert!(offset_of!(KinfoProc, kp_eproc.e_ppid) == 560);
    };

    /// How many times the `KERN_PROC_ALL` fetch re-asks for a size after
    /// the table grew between the sizing call and the fetch.
    ///
    /// The race is inherent to the two-call `sysctl` idiom (ask how big,
    /// then read) and is ordinary on a busy host, not pathological — each
    /// retry re-sizes against a fresher table, so convergence is the normal
    /// case and this bound is only a guard against livelock under a fork
    /// storm. Exhausting it is a hard [`snapshot`] error, never an empty
    /// map, per this module's fail-closed contract.
    const MAX_SIZE_RETRIES: usize = 5;

    /// Slack rows added on top of the size `sysctl` reported, so the common
    /// case of a handful of processes starting between the two calls costs
    /// nothing instead of a retry.
    const SIZE_HEADROOM_ROWS: usize = 32;

    /// The process's own effective uid — the scope [`snapshot`] restricts
    /// itself to, mirroring the Linux walk's `hidepid` skip.
    fn euid() -> libc::uid_t {
        // SAFETY: geteuid takes no arguments and cannot fail.
        unsafe { libc::geteuid() }
    }

    /// The start-time identity token for one row: the process's start
    /// `timeval` folded to whole microseconds.
    ///
    /// Injective and stable over a process's life, which is all the sweep's
    /// pid-reuse check needs — and it must be computed here and ONLY here,
    /// because [`snapshot`] and [`read_process`] compare their results
    /// against each other. Clamping the components at zero rather than
    /// casting blindly keeps a nonsensical negative from the kernel (which
    /// should never happen) from wrapping into a value that could collide
    /// with a real one.
    fn starttime_key(start: &libc::timeval) -> u64 {
        let secs = start.tv_sec.max(0) as u64;
        let usecs = start.tv_usec.max(0) as u64;
        secs.saturating_mul(1_000_000).saturating_add(usecs)
    }

    /// Classify one row's `p_stat`. `SZOMB` (from `<sys/proc.h>`) is the
    /// value a process wears once it has exited and is waiting to be
    /// reaped; every other value — running, sleeping, stopped, idle —
    /// means it can still execute code, which is the only distinction the
    /// sweep draws.
    fn state_of(row: &KinfoProc) -> ProcessState {
        if row.kp_proc.p_stat as u32 == libc::SZOMB {
            ProcessState::Zombie
        } else {
            ProcessState::Running
        }
    }

    /// Fetch `kern.proc` rows for one selector (`KERN_PROC_ALL` with `arg`
    /// 0, or `KERN_PROC_PID` with a pid).
    ///
    /// Sizes with a `NULL` probe, allocates with headroom, and retries on
    /// `ENOMEM` — the classic Darwin process-table race, where the answer
    /// outgrows the buffer between the two calls. An empty result is a
    /// legitimate answer (`KERN_PROC_PID` for a pid that no longer exists),
    /// not an error.
    fn kern_proc(selector: c_int, arg: c_int) -> std::io::Result<Vec<KinfoProc>> {
        let mut mib: [c_int; 4] = [libc::CTL_KERN, libc::KERN_PROC, selector, arg];
        for _ in 0..MAX_SIZE_RETRIES {
            let mut needed: libc::size_t = 0;
            // SAFETY: `mib` is a live array of exactly the 4 elements
            // declared to sysctl; a NULL `oldp` with a non-NULL `oldlenp`
            // is the documented "how big is the answer" form and writes
            // only through `needed`.
            let sized = unsafe {
                libc::sysctl(
                    mib.as_mut_ptr(),
                    4,
                    std::ptr::null_mut(),
                    &mut needed,
                    std::ptr::null_mut(),
                    0,
                )
            };
            if sized != 0 {
                return Err(std::io::Error::last_os_error());
            }
            if needed == 0 {
                return Ok(Vec::new());
            }

            let rows = needed.div_ceil(size_of::<KinfoProc>()) + SIZE_HEADROOM_ROWS;
            let mut buf: Vec<KinfoProc> = Vec::with_capacity(rows);
            let mut len = rows * size_of::<KinfoProc>();
            // SAFETY: `buf` has capacity for `rows` rows and `len` is
            // exactly that many bytes, so sysctl writes only within the
            // allocation; it reports how much it actually wrote back
            // through `len`, and `set_len` below admits only whole rows the
            // kernel initialized. Every field of `KinfoProc` is a plain
            // integer, array, or raw pointer, so any initialized bit
            // pattern is a valid value.
            let fetched = unsafe {
                libc::sysctl(
                    mib.as_mut_ptr(),
                    4,
                    buf.as_mut_ptr().cast::<c_void>(),
                    &mut len,
                    std::ptr::null_mut(),
                    0,
                )
            };
            if fetched != 0 {
                let err = std::io::Error::last_os_error();
                if err.raw_os_error() == Some(libc::ENOMEM) {
                    // The table grew past even the headroom: re-size and
                    // try again rather than reporting a failure for what is
                    // just a busy host.
                    continue;
                }
                return Err(err);
            }
            // SAFETY: sysctl wrote `len` bytes into the allocation; whole
            // rows within that are initialized.
            unsafe { buf.set_len(len / size_of::<KinfoProc>()) };
            return Ok(buf);
        }
        Err(std::io::Error::other(format!(
            "the kern.proc table kept outgrowing its buffer across {MAX_SIZE_RETRIES} attempts"
        )))
    }

    /// See [`super::snapshot`]. One `KERN_PROC_ALL` sysctl, filtered to
    /// this euid.
    ///
    /// The soft-error list is always empty here and that is not an
    /// oversight: the whole table arrives in a single syscall, so there is
    /// no per-process read that could fail on its own. Either the table was
    /// read or it was not.
    ///
    /// pid 0 (the kernel task) is dropped rather than filtered by uid: it
    /// runs as root on every Mac, so the euid filter already excludes it
    /// for an ordinary user, and dropping it explicitly means a supervisor
    /// running as root cannot end up with it as a closure root.
    pub(super) fn snapshot() -> Result<(super::ProcessTable, Vec<String>), String> {
        let rows = kern_proc(libc::KERN_PROC_ALL, 0)
            .map_err(|e| format!("reading the kern.proc process table: {e}"))?;
        let me = euid();
        let mut stats = HashMap::new();
        for row in &rows {
            let pid = row.kp_proc.p_pid;
            if pid <= 0 || row.kp_eproc.e_ucred.cr_uid != me {
                continue;
            }
            let ppid = row.kp_eproc.e_ppid.max(0) as u32;
            stats.insert(pid as u32, (ppid, starttime_key(&row.kp_proc.p_starttime)));
        }
        Ok((stats, Vec::new()))
    }

    /// See [`super::read_process`]. One `KERN_PROC_PID` sysctl.
    ///
    /// An empty answer means the pid is gone, and so does `ESRCH`: Darwin
    /// has answered both ways for a vanished pid across releases, and the
    /// two mean the same thing to the sweep. No uid filter is needed here: a
    /// readable row for a foreign replacement still carries the start time
    /// callers compare against the recorded identity, while a sysctl failure
    /// remains an error rather than being mistaken for absence.
    pub(super) fn read_process(pid: u32) -> Result<Option<(u32, u64, ProcessState)>, String> {
        let rows = match kern_proc(libc::KERN_PROC_PID, pid as c_int) {
            Ok(rows) => rows,
            Err(e) if e.raw_os_error() == Some(libc::ESRCH) => return Ok(None),
            Err(e) => return Err(format!("reading kern.proc for pid {pid}: {e}")),
        };
        let Some(row) = rows.first() else {
            return Ok(None);
        };
        Ok(Some((
            row.kp_eproc.e_ppid.max(0) as u32,
            starttime_key(&row.kp_proc.p_starttime),
            state_of(row),
        )))
    }

    /// The kernel's `kern.argmax`: the hard ceiling on one process's
    /// combined argv and environment, and therefore on any
    /// `KERN_PROCARGS2` answer.
    ///
    /// This is [`read_environ`]'s ONLY buffer sizing, not a fallback, and
    /// that is a correctness requirement rather than a simplification: XNU
    /// has a long-standing `KERN_PROCARGS2` bug (observed on this
    /// project's own hardware, macOS 26.5.1) where the NULL-`oldp` size probe
    /// underestimates — it fails to count the 17-byte `executable_path=`
    /// prefix — and a fetch into a buffer of exactly the probed size does
    /// not fail: it SUCCEEDS and fills the buffer with zeros. A zero-fill
    /// parses as an empty environment, which silently disabled the entire
    /// marker half of the sweep. Apple's own `ps` has always sized this
    /// buffer from `kern.argmax` instead of probing, and crashpad and
    /// golang's x/sys carry explicit workarounds for the same bug:
    /// <https://groups.google.com/a/chromium.org/g/crashpad-dev/c/ASKdHGWG5bA>,
    /// <https://github.com/golang/go/issues/60047>. Since the args area
    /// can never exceed argmax by definition, an argmax-sized buffer can
    /// never be "too small" and the buggy path is unreachable.
    ///
    /// Cached because it is fixed for the life of the boot while
    /// [`read_environ`] runs once per process per sweep round. A failed
    /// read falls back to a generous fixed size rather than giving up —
    /// deliberately NOT the 4 KiB POSIX `ARG_MAX` floor, which on a host
    /// with a bigger real argmax would recreate the undersized-buffer
    /// zero-fill above. One megabyte is `kern.argmax`'s actual value on
    /// every macOS this project has met.
    fn arg_max() -> usize {
        static CACHED: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
        *CACHED.get_or_init(|| {
            let mut mib: [c_int; 2] = [libc::CTL_KERN, libc::KERN_ARGMAX];
            let mut value: c_int = 0;
            let mut len = size_of::<c_int>();
            // SAFETY: `mib` holds exactly the 2 elements declared, and the
            // out-buffer is one live `c_int` with `len` saying so.
            let ok = unsafe {
                libc::sysctl(
                    mib.as_mut_ptr(),
                    2,
                    (&raw mut value).cast::<c_void>(),
                    &mut len,
                    std::ptr::null_mut(),
                    0,
                )
            } == 0;
            if ok && value > 0 {
                value as usize
            } else {
                1024 * 1024
            }
        })
    }

    /// See [`super::read_environ`]. One `kern.procargs2` sysctl, parsed by
    /// [`super::parse_procargs2`] — which is where the argv region gets
    /// dropped, and where the reasoning for dropping it lives.
    ///
    /// Sized from [`arg_max`], never from the NULL-`oldp` probe. A probe
    /// looks like the ordinary idiom and is exactly wrong for THIS node:
    /// XNU's estimate comes back short, and a fetch into a buffer of the
    /// probed size succeeds while writing zeros — see [`arg_max`]'s docs
    /// for the bug's shape and references. The first cut here probed, and
    /// on real hardware every environment came back empty: the sweep's
    /// marker half was silently disabled while every test that does not
    /// spawn a real process still passed.
    ///
    /// Every failure collapses to `None`: Darwin answers `EINVAL` for a
    /// process this one does not own (there is no readable-but-empty case
    /// to distinguish), `ESRCH` for one that exited, and the caller treats
    /// all of it as "no marker" regardless.
    pub(super) fn read_environ(pid: u32) -> Option<Vec<u8>> {
        let mut mib: [c_int; 3] = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid as c_int];
        let needed = arg_max();
        let mut buf = vec![0u8; needed];
        let mut len = needed;
        // SAFETY: `buf` owns `needed` bytes and `len` says exactly that, so
        // sysctl writes only within the allocation; `len` comes back as the
        // number of bytes actually written, and the truncate below keeps
        // the parser away from anything the kernel did not fill in.
        let fetched = unsafe {
            libc::sysctl(
                mib.as_mut_ptr(),
                3,
                buf.as_mut_ptr().cast::<c_void>(),
                &mut len,
                std::ptr::null_mut(),
                0,
            )
        };
        if fetched != 0 {
            return None;
        }
        buf.truncate(len);
        super::parse_procargs2(&buf)
    }

    /// The raw bytes of `proc_pidpath`: the same source the emitter
    /// check has always read, exposed so the shared walk can capture the
    /// image once per edge instead of every consumer re-reading it.
    pub(super) fn process_exe(pid: u32) -> Result<Vec<u8>, String> {
        let pid = i32::try_from(pid).map_err(|_| "process PID is out of range".to_string())?;
        let mut path = [0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
        // SAFETY: the kernel receives the buffer's exact writable capacity.
        let len = unsafe { libc::proc_pidpath(pid, path.as_mut_ptr().cast(), path.len() as u32) };
        if len <= 0 {
            return Err(format!(
                "reading process {pid} executable: {}",
                std::io::Error::last_os_error()
            ));
        }
        let bytes = &path[..len as usize];
        Ok(bytes.strip_suffix(&[0]).unwrap_or(bytes).to_vec())
    }

    /// Whether raw `proc_pidpath` bytes name a native Codex image: the
    /// basename check the emitter has always applied, over bytes the
    /// caller already captured.
    pub(super) fn is_codex_image(exe: &[u8]) -> bool {
        exe.rsplit(|byte| *byte == b'/').next() == Some(b"codex".as_slice())
    }

    /// The bounded argv region of a fresh `KERN_PROCARGS2` buffer: the
    /// same sysctl fetch the environ reader performs, parsed for argv
    /// instead of environ. Capped, truncated-safe, and `None` on every
    /// failure exactly like the environ reader's contract.
    pub(super) fn read_process_argv(pid: u32) -> Option<Vec<Vec<u8>>> {
        let pid = i32::try_from(pid).ok()?;
        let mut mib: [c_int; 3] = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid as c_int];
        // The per-process argv cap bounds this fetch, not ARG_MAX: there
        // is no reason to copy megabytes of args to keep kilobytes of
        // argv, and a buffer sized to the cap makes truncation
        // observable (a full buffer with no room left for the parser's
        // terminator) instead of silent.
        let needed = super::MAX_ARGV_BYTES_PER_PROCESS + 1;
        let mut buf = vec![0u8; needed];
        let mut len = needed;
        // SAFETY: `buf` owns `needed` bytes and `len` says exactly that,
        // so sysctl writes only within the allocation; `len` comes back
        // as the number of bytes actually written, and the truncate below
        // keeps the parser away from anything the kernel did not fill in.
        let fetched = unsafe {
            libc::sysctl(
                mib.as_mut_ptr(),
                3,
                buf.as_mut_ptr().cast::<c_void>(),
                &mut len,
                std::ptr::null_mut(),
                0,
            )
        };
        if fetched != 0 {
            return None;
        }
        buf.truncate(len);
        // A too-small buffer surfaces as ENOMEM above, never as silent
        // truncation — so a successful fetch holds the whole argv region,
        // and anything the parser refuses past this point is malformed
        // data, not a short read.
        super::parse_procargs2_argv(&buf)
    }
}

/// A marked, long-lived child process for kill-sweep and environ tests —
/// THIS test binary re-invoked in a sleeper mode, never a system binary.
///
/// Why the contortion instead of `sh -c "sleep 30"`: macOS 26+ withholds
/// the entire environment region of `KERN_PROCARGS2` when the target's
/// exec'd image is an Apple PLATFORM binary — observed on macOS 26.5.1,
/// where `/bin/sleep` and `sh` children answered argv-only (`len` covering
/// exactly argc + exec path + argv) while the same read of a locally
/// compiled child returned the full environment. A test child that the
/// marker scan cannot see does not exercise the marker mechanism; it
/// proves nothing and fails on every modern Mac. The one binary a test can
/// rely on being locally built — and therefore readable — is the test
/// executable itself, which libtest happily re-invokes as a single-test
/// child. (The withholding itself is pinned by its own test below, so if
/// Apple ever lifts it, the record gets corrected rather than silently
/// drifting.)
///
/// The child announces itself on stdout before sleeping and [`spawn`]
/// waits for that line. The handshake is load-bearing, not tidiness:
/// `spawn` returns once the child EXISTS, not once it has finished
/// `exec`ing, and a pre-exec child still shows its parent's environment —
/// under load that read raced and failed for reasons unrelated to what any
/// caller pins. It also confirms libtest really reached the sleeper test,
/// rather than exiting early over an argument-parsing change.
#[cfg(test)]
pub(crate) mod sleeper {
    /// Printed by the sleeper child once its own test body is running —
    /// i.e., strictly after `exec`, when its environment is its own.
    const READY_LINE: &str = "SLEEPER_READY";

    /// The env var that turns the sleeper test below into an actual
    /// sleeper. Set on the CHILD only (`Command::env`); the test process's
    /// own environment is never touched (a repo-wide rule).
    const SLEEPER_MODE_ENV: &str = "FARHELM_TEST_SLEEPER_MODE";

    /// Set on a sleeper CHILD (via `extra_env`) to make it ignore SIGTERM,
    /// the way an interactive shell does: the fixture for the sweep's
    /// hang-up of tab processes. Like [`SLEEPER_MODE_ENV`] it is only ever
    /// set on a child's command, never in the test process's environment.
    pub(crate) const SLEEPER_IGNORE_TERM_ENV: &str = "FARHELM_TEST_SLEEPER_IGNORE_TERM";

    /// Build the sleeper's command: the test binary re-invoked in sleeper
    /// mode, carrying exactly the farhelm markers `extra_env` declares and
    /// no others.
    ///
    /// The ambient-marker scrub is the load-bearing part. The test runner
    /// itself may be running inside a Farhelm session — the ordinary state
    /// of an agent developing farhelm on a farhelm-supervised box — where
    /// `FARHELM_AGENT_ID` (or, from a tab, `FARHELM_TAB_ID`) sits in the
    /// runner's own environment. A sleeper inheriting a marker its caller
    /// never declared is no longer the shape the caller meant to spawn:
    /// the sweep tests' marked process, declared with a session marker
    /// only, picked up the host session's agent marker and read to the
    /// sweep as another launch's agent — which the cross-session boundary
    /// correctly refuses, failing four sweep tests on such hosts while CI
    /// (marker-free) stayed green. Same bug class, and same child-only
    /// remedy, as `MarkedDecoy::command`'s scrub in the e2e harness.
    /// `env_remove` runs before `extra_env` is applied, so a test that
    /// WANTS a marker still gets it by declaring it.
    ///
    /// Split from [`spawn`] so a test can assert the CONFIGURED env ops
    /// via `Command::get_envs`: the scrub only changes a live child's
    /// environment on a host whose runner carries the markers, so
    /// inspecting the child would prove nothing on clean CI, while the
    /// builder's op list is the same everywhere.
    fn command(extra_env: &[(&str, &str)]) -> std::process::Command {
        let exe = std::env::current_exe().expect("the test binary knows its own path");
        let mut cmd = std::process::Command::new(exe);
        // `--exact` addresses the one test by its full libtest path;
        // `--nocapture` is what lets its READY line reach the pipe instead
        // of libtest's capture buffer.
        cmd.args([
            "--exact",
            "procs::sleeper::runs_as_a_sleeper_child_when_asked",
            "--nocapture",
        ]);
        cmd.env(SLEEPER_MODE_ENV, "1");
        cmd.env_remove(crate::launch::SESSION_ID_ENV_VAR);
        cmd.env_remove(crate::launch::AGENT_ID_ENV_VAR);
        cmd.env_remove(crate::launch::TAB_ID_ENV_VAR);
        for (key, value) in extra_env {
            cmd.env(key, value);
        }
        cmd
    }

    /// Spawn the sleeper with `extra_env` in its environment, returning
    /// once it is provably past `exec`. The caller owns `kill`/`wait`;
    /// clippy's zombie lint cannot see across that handoff, and the panic
    /// paths in here leak at most one 30-second sleeper.
    #[allow(clippy::zombie_processes)]
    pub(crate) fn spawn(extra_env: &[(&str, &str)]) -> std::process::Child {
        use std::io::BufRead as _;
        let mut child = command(extra_env)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("re-spawning the test binary as a sleeper");
        let stdout = child.stdout.take().expect("a piped stdout was requested");
        // libtest prints its own header lines first; scan until the
        // sleeper speaks. EOF before that means it never started sleeping
        // (typically a libtest CLI change) — fail here, loudly, rather
        // than let the caller's assertion fire for the wrong reason.
        let mut lines = std::io::BufReader::new(stdout).lines();
        loop {
            let line = lines
                .next()
                .expect("the sleeper child exited before announcing readiness")
                .expect("reading the sleeper child's stdout");
            if line.trim() == READY_LINE {
                return child;
            }
        }
    }

    /// The sleeper's builder must scrub every farhelm marker the runner
    /// could ambiently carry, and a declared marker must survive the
    /// scrub.
    ///
    /// This pins the fix for the four sweep tests that failed only when
    /// the suite ran inside a Farhelm session (see [`command`]'s docs for
    /// the mechanism). Asserted against `Command::get_envs` — the
    /// CONFIGURED operations — because a live child's environment only
    /// differs from a clean one on a polluted host, so this is the one
    /// observation that fails the same way everywhere, CI included.
    #[farhelm_testtrace::test]
    fn sleeper_command_scrubs_ambient_markers_but_keeps_declared_ones() {
        let cmd = command(&[(crate::launch::SESSION_ID_ENV_VAR, "declared-session")]);
        let configured: std::collections::HashMap<_, _> = cmd
            .get_envs()
            .map(|(key, value)| (key.to_os_string(), value.map(|v| v.to_os_string())))
            .collect();
        assert_eq!(
            configured
                .get(std::ffi::OsStr::new(crate::launch::SESSION_ID_ENV_VAR))
                .cloned(),
            Some(Some("declared-session".into())),
            "a marker the caller declares must survive the scrub"
        );
        for removed in [
            crate::launch::AGENT_ID_ENV_VAR,
            crate::launch::TAB_ID_ENV_VAR,
        ] {
            assert_eq!(
                configured.get(std::ffi::OsStr::new(removed)).cloned(),
                Some(None),
                "{removed} must be configured as removed, or a runner inside a farhelm \
                 session leaks it into every sleeper"
            );
        }
    }

    /// Not a test of anything: the body [`spawn`]'s children execute. In
    /// an ordinary suite run the mode env var is absent and this returns
    /// immediately as a trivially green test; as a spawned child it
    /// announces readiness and sleeps. The 30s bound is the leak ceiling
    /// if a caller panics before its sweep runs, same as the `sleep 30`
    /// it replaced. Keep this stand-in outside automatic trace capture: its
    /// expected termination belongs to the parent test, and cannot complete
    /// an independent capture session.
    #[test]
    fn runs_as_a_sleeper_child_when_asked() {
        if std::env::var_os(SLEEPER_MODE_ENV).is_none() {
            return;
        }
        use std::io::Write as _;
        if std::env::var_os(SLEEPER_IGNORE_TERM_ENV).is_some() {
            // Installed BEFORE the readiness line, so a parent that
            // signals the moment it reads READY can never catch the
            // default disposition instead of the ignoring one.
            //
            // SAFETY: `signal(2)` needs only a valid signal number and
            // disposition; `SIG_IGN` installs no handler code.
            unsafe {
                libc::signal(libc::SIGTERM, libc::SIG_IGN);
            }
        }
        println!("{READY_LINE}");
        std::io::stdout()
            .flush()
            .expect("flushing the readiness line");
        // sleep-ok: hold the announced fixture process alive for the parent's sweep, with a finite lifetime if its owner fails.
        std::thread::sleep(std::time::Duration::from_secs(30));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a `KERN_PROCARGS2` buffer the way the Darwin kernel lays one
    /// out, so the parser is exercised against the real shape rather than
    /// against its own assumptions: `argc`, the exec path, alignment
    /// padding, argv, then the environment.
    fn procargs2(argv: &[&str], environ: &[&str], padding: usize) -> Vec<u8> {
        let mut buf = (argv.len() as i32).to_ne_bytes().to_vec();
        buf.extend_from_slice(b"/usr/bin/thing\0");
        buf.extend(std::iter::repeat_n(0u8, padding));
        for arg in argv {
            buf.extend_from_slice(arg.as_bytes());
            buf.push(0);
        }
        for entry in environ {
            buf.extend_from_slice(entry.as_bytes());
            buf.push(0);
        }
        buf
    }

    /// The parser's contract in one pass: the environment region comes back
    /// as a NUL-delimited block, and the exec path, the alignment padding,
    /// and every argv element are gone from it.
    ///
    /// Worth pinning as its own test because the argv skip is counted, not
    /// searched for: an off-by-one in the padding skip or the argc loop
    /// shifts the boundary and silently leaks command-line bytes into what
    /// the sweep treats as a process's environment.
    #[farhelm_testtrace::test]
    fn procargs2_parsing_yields_the_environment_region_alone() {
        let buf = procargs2(
            &["thing", "--flag", "value"],
            &["PATH=/bin", "FARHELM_SESSION_ID=abc-123"],
            7,
        );
        let environ = parse_procargs2(&buf).expect("a well-formed buffer must parse");
        assert_eq!(environ, b"PATH=/bin\0FARHELM_SESSION_ID=abc-123\0");
    }

    /// The reason argv is excluded at all: a process whose COMMAND LINE
    /// carries a session marker must not be claimed by that session's
    /// sweep.
    ///
    /// This is the platform-agreement claim, and it is a safety one. Linux
    /// reads `/proc/<pid>/environ`, which can never contain argv; macOS
    /// reads a buffer that holds both. If the argv region leaked through,
    /// `grep FARHELM_SESSION_ID=<id> logfile` — or an editor with such a
    /// file open — would be SIGKILLed by a stop it has nothing to do with,
    /// on macOS only.
    #[farhelm_testtrace::test]
    fn a_marker_on_the_command_line_never_reaches_the_environment_block() {
        let marker = "FARHELM_SESSION_ID=abc-123";
        let buf = procargs2(&["grep", marker, "app.log"], &["PATH=/bin"], 3);
        let environ = parse_procargs2(&buf).expect("a well-formed buffer must parse");
        assert_eq!(environ, b"PATH=/bin\0");
        assert!(
            !environ
                .windows(marker.len())
                .any(|w| w == marker.as_bytes()),
            "argv bytes must not appear anywhere in the environment block"
        );
    }

    /// An empty entry ends the environment region, and a truncated trailing
    /// entry is dropped rather than guessed at.
    ///
    /// Both matter for the same reason: the block this returns is fed
    /// straight to exact, whole-entry marker matching, so a half-read entry
    /// admitted as if it were complete could match a marker whose value was
    /// cut short — claiming a process for the wrong session.
    #[farhelm_testtrace::test]
    fn procargs2_parsing_stops_at_an_empty_entry_and_drops_a_truncated_tail() {
        let mut buf = procargs2(&["thing"], &["A=1"], 1);
        buf.push(0); // the empty entry that ends the region
        buf.extend_from_slice(b"B=2\0");
        assert_eq!(parse_procargs2(&buf).unwrap(), b"A=1\0");

        let mut truncated = procargs2(&["thing"], &["A=1"], 1);
        truncated.extend_from_slice(b"FARHELM_SESSION_ID=abc"); // no NUL
        assert_eq!(parse_procargs2(&truncated).unwrap(), b"A=1\0");
    }

    /// Structurally unusable buffers must answer `None` (which the caller
    /// reads as "no marker") rather than panicking or inventing an
    /// environment out of whatever bytes are present. A short read from a
    /// process exiting mid-sysctl is the realistic source.
    #[farhelm_testtrace::test]
    fn procargs2_parsing_rejects_buffers_it_cannot_trust() {
        assert!(parse_procargs2(b"").is_none());
        assert!(parse_procargs2(b"\x01\x00").is_none(), "argc is truncated");
        assert!(
            parse_procargs2(b"\x01\x00\x00\x00/usr/bin/thing").is_none(),
            "the exec path has no terminator, so nothing after it can be located"
        );
        // argc claims more argv entries than the buffer holds: an empty
        // environment, not a parse failure.
        let buf = procargs2(&["a"], &[], 1);
        let mut greedy = 9i32.to_ne_bytes().to_vec();
        greedy.extend_from_slice(&buf[4..]);
        assert_eq!(parse_procargs2(&greedy).unwrap(), b"");
    }

    /// This process must be able to find ITSELF in the table, with its own
    /// parent and a start time that matches what the single-pid read
    /// reports.
    ///
    /// The agreement between the two reads is the point. `sweep` records a
    /// start time from [`snapshot`] and re-checks it via [`read_process`]
    /// immediately before signaling; if a platform derived the two
    /// differently, every validated signal would be skipped and the sweep
    /// would silently stop killing anything while still reporting success.
    #[farhelm_testtrace::test]
    fn snapshot_and_single_reads_agree_about_this_process() {
        let me = std::process::id();
        let (stats, soft_errors) = snapshot().expect("this host's process table must be readable");
        let &(ppid, starttime) = stats
            .get(&me)
            .expect("the walk must contain the process performing it");
        let (single_ppid, single_starttime, state) = read_process(me)
            .expect("reading this process must not error")
            .expect("this process exists");
        assert_eq!(ppid, single_ppid, "both reads must report the same parent");
        assert_eq!(
            starttime, single_starttime,
            "both reads must derive start time identically, or every signal is skipped"
        );
        assert_eq!(state, ProcessState::Running);
        assert!(
            soft_errors.is_empty() || !stats.is_empty(),
            "soft errors must never come at the cost of an empty table: {soft_errors:?}"
        );
    }

    /// A pid that cannot exist is `Ok(None)` — gone — and never an error.
    ///
    /// The distinction is the whole reason the return type is nested:
    /// `confirm_gone` counts `Ok(None)` as a process successfully reaped
    /// and treats `Err` as an unconfirmed kill that fails the stop, so
    /// collapsing them in either direction breaks a user-visible promise.
    #[farhelm_testtrace::test]
    fn an_impossible_pid_reads_as_gone_rather_than_as_an_error() {
        // Above every platform's pid_max, so it can never name a process.
        assert_eq!(read_process(u32::from(u16::MAX) * 1024 + 7), Ok(None));
    }

    /// The environment read must return the marker block the sweep matches
    /// on, for a process whose environment this test controls — the seam's
    /// end-to-end claim on whichever platform it runs.
    ///
    /// The child is the test binary itself in sleeper mode, not a system
    /// binary, and on macOS that choice is what the test MEANS: see
    /// [`super::sleeper`] for the platform-binary withholding that makes a
    /// `sh` child unreadable there, and the companion macOS test below
    /// that pins the withholding itself.
    #[farhelm_testtrace::test]
    fn a_childs_exec_time_environment_is_readable_as_nul_delimited_entries() {
        let mut child = sleeper::spawn(&[("FARHELM_PROCS_TEST_MARKER", "sentinel-value")]);
        let environ = read_environ(child.id()).expect("a child's environment must be readable");
        let found = environ
            .split(|&b| b == 0)
            .any(|entry| entry == b"FARHELM_PROCS_TEST_MARKER=sentinel-value");
        let _ = child.kill();
        let _ = child.wait();
        assert!(
            found,
            "the exec-time environment must come back as complete NUL-delimited entries"
        );
    }

    /// macOS 26+ withholds the ENVIRONMENT region of `KERN_PROCARGS2` for
    /// Apple platform binaries, even for a same-uid direct child: the
    /// fetch succeeds and answers argv-only. Observed on macOS 26.5.1
    /// (2026-08): a `/bin/sleep` child answered with `len` covering
    /// exactly argc + exec path + argv, while the same read of this test
    /// binary's own child returned the full environment.
    ///
    /// This pins a LIMITATION, deliberately: the sweep's marker scan
    /// cannot see a reparented descendant whose exec'd image is a platform
    /// binary (shells, chiefly), which is an accepted residual documented
    /// on `sweep::environ_markers_of` — the deferred follow-up is a
    /// session-id membership channel. If this test ever FAILS, Apple has
    /// started returning environments for platform binaries again: good
    /// news, and the cue to update that documentation rather than a bug.
    #[cfg(target_os = "macos")]
    #[farhelm_testtrace::test]
    fn a_platform_binary_childs_environment_is_withheld_on_modern_macos() {
        let mut child = std::process::Command::new("/bin/sleep")
            .arg("30")
            .env("FARHELM_PROCS_TEST_MARKER", "sentinel-value")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("spawning /bin/sleep");
        // No handshake is possible with a child that prints nothing, so
        // poll past the exec window instead; the marker CANNOT appear
        // pre-exec either (the test process's own environment does not
        // carry it), so a positive sighting is definitive whenever it
        // lands.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        let mut marker_seen = false;
        while std::time::Instant::now() < deadline {
            if let Some(environ) = read_environ(child.id()) {
                marker_seen = environ
                    .split(|&b| b == 0)
                    .any(|entry| entry == b"FARHELM_PROCS_TEST_MARKER=sentinel-value");
                if marker_seen {
                    break;
                }
            }
            // sleep-ok: sample marker visibility throughout the negative observation window; one absent pre-exec read is insufficient.
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let _ = child.kill();
        let _ = child.wait();
        assert!(
            !marker_seen,
            "macOS returned a platform binary's environment — the withholding this test \
             pins has been lifted; update the marker-scan residual docs accordingly"
        );
    }

    /// The argv parser's contract in one pass: exactly `argc`
    /// NUL-terminated entries after the exec path and its padding, with
    /// the environment region left for the environ parser.
    ///
    /// Why this test matters: this is the entry-point evidence the later
    /// per-kind proofs match against. An off-by-one in the padding skip
    /// or the argc loop would shift every entry — matching the WRONG
    /// argv[0] to a runtime — while still returning `Some`, the failure
    /// direction no caller could detect.
    #[farhelm_testtrace::test]
    fn procargs2_argv_parsing_yields_exactly_argc_entries() {
        let buf = procargs2(
            &["thing", "--flag", "value"],
            &["PATH=/bin", "FARHELM_SESSION_ID=abc-123"],
            7,
        );
        let argv = parse_procargs2_argv(&buf).expect("a well-formed buffer must parse");
        let argv: Vec<&[u8]> = argv.iter().map(Vec::as_slice).collect();
        assert_eq!(
            argv,
            [
                b"thing".as_slice(),
                b"--flag".as_slice(),
                b"value".as_slice()
            ]
        );
    }

    /// Truncation and over-budget argv are refused as evidence, never
    /// returned as a prefix.
    ///
    /// Why this test matters: the two failure directions have opposite
    /// safety properties. Returning the readable prefix as the whole argv
    /// would let a cut-short observation match an entry point the full
    /// command line never named; `None` forces the proof to treat the
    /// evidence as missing instead.
    #[farhelm_testtrace::test]
    fn procargs2_argv_parsing_refuses_truncation_and_over_budget_reads() {
        let buf = procargs2(&["thing", "--flag"], &["PATH=/bin"], 3);
        // Cut mid-second-entry ("--fl" with no terminator after it): the
        // parser must not return ["thing"]. The cut point is computed
        // from the layout — argc, exec path, padding, first entry —
        // rather than from the buffer end, which would land in the
        // environment region the argv parser never reads.
        let cut = 4 + "/usr/bin/thing\0".len() + 3 + "thing\0".len() + "--fl".len();
        assert_eq!(&buf[cut - 4..cut], b"--fl");
        assert!(
            parse_procargs2_argv(&buf[..cut]).is_none(),
            "a buffer ending mid-argv is truncated evidence, not a one-entry argv"
        );
        // argc claiming more entries than the buffer holds: same refusal.
        let mut greedy = 9i32.to_ne_bytes().to_vec();
        greedy.extend_from_slice(&buf[4..]);
        assert!(
            parse_procargs2_argv(&greedy).is_none(),
            "a greedy argc must not invent entries past the buffer"
        );
        // Structurally unusable buffers answer None, like the environ
        // parser's rejection test pins for its region.
        assert!(parse_procargs2_argv(b"").is_none());
        assert!(parse_procargs2_argv(b"\x01\x00").is_none());
        let over = procargs2(&["thing"], &[], 1);
        // Splice an over-budget second entry into the ARGV region (argc
        // 2): the parser must refuse the total, not return the readable
        // prefix. Appending past the environ region would not exercise
        // the cap at all, since the parser stops after argc entries.
        let mut over_argv = (2i32).to_ne_bytes().to_vec();
        over_argv.extend_from_slice(&over[4..]);
        let insert_at = over_argv.len();
        over_argv.extend(std::iter::repeat_n(b'x', MAX_ARGV_BYTES_PER_PROCESS));
        over_argv.push(0);
        assert!(insert_at > 4, "the splice must land past the header");
        assert!(
            parse_procargs2_argv(&over_argv).is_none(),
            "an argv total past the per-process cap is refused, not returned"
        );
    }

    /// The live argv reader returns this test process's own command line
    /// with its executable first — the reader's end-to-end claim on
    /// whichever platform it runs.
    ///
    /// Why this test matters: the synthetic parser tests above pin the
    /// format, but only a live read proves the platform fetch feeds the
    /// parser the right region (argv, not environ, not the exec path).
    /// Reading the test process itself keeps the fixture premise inside
    /// the test's own lifetime: no child to reap, no lifetime to race.
    #[farhelm_testtrace::test]
    fn a_process_argv_read_returns_the_live_command_line() {
        let me = std::process::id();
        let argv = read_process_argv(me).expect("a live same-uid process must expose argv");
        assert!(
            !argv.is_empty() && !argv[0].is_empty(),
            "argv[0] must name the running image"
        );
    }

    /// The shared walk attributes the live test process to itself as a
    /// one-edge chain with image and argv evidence attached.
    ///
    /// Why this test matters: [`walk_to_pane`] is the mechanics every
    /// per-kind proof builds on, and a single-edge walk exercises its
    /// whole contract — peer token check, live-state requirement, image
    /// capture, argv capture, pane termination — without a fixture pane
    /// whose lifetime the test would have to defend.
    #[farhelm_testtrace::test]
    fn the_shared_walk_attributes_a_live_process_to_itself() {
        let me = std::process::id();
        let peer = ProcessIdentity::read(me).expect("this process is live");
        let chain = walk_to_pane(peer, me).expect("a live process must walk to itself");
        assert_eq!(chain.len(), 1, "no ancestors are climbed past the pane");
        let link = &chain[0];
        assert_eq!(link.pid, me);
        assert_eq!(link.start, peer.start);
        assert!(!link.exe.is_empty(), "the image is required evidence");
        assert!(
            link.argv.as_ref().is_some_and(|argv| !argv.is_empty()),
            "a live same-uid process must expose argv evidence"
        );
    }

    /// One synthetic chain link: pid/start are distinct per link so an
    /// admitted emitter is identifiable as the intended process, and argv
    /// is spelled exactly as the kernel would capture it (argv[0] is the
    /// executable spelling, never validated).
    fn corridor_link(pid: u32, exe: &str, argv: &[&str]) -> ChainLink {
        ChainLink {
            pid,
            ppid: 0,
            start: u64::from(pid) * 1_000,
            exe: exe.as_bytes().to_vec(),
            argv: Some(argv.iter().map(|arg| arg.as_bytes().to_vec()).collect()),
        }
    }

    fn corridor_link_no_argv(pid: u32, exe: &str) -> ChainLink {
        ChainLink {
            pid,
            ppid: 0,
            start: u64::from(pid) * 1_000,
            exe: exe.as_bytes().to_vec(),
            argv: None,
        }
    }

    /// The installed hook command, as the kernel captures it on the
    /// reporter: the executable spelling plus the verb sequence. Vendors
    /// run this string through a shell, so the same spelling appears both
    /// on direct children and inside a trampoline's `-c` command.
    fn hook_argv() -> Vec<&'static str> {
        vec![
            "/opt/test/bin/farhelm",
            "internal",
            "hook",
            "--vendor",
            "codex",
        ]
    }

    /// The direct-child shape every production hook takes: no
    /// intermediary between the reporter and the runtime.
    ///
    /// This is the shape the corridor must keep admitting — the fix for
    /// unclassified intermediaries below must not narrow it.
    #[farhelm_testtrace::test]
    fn a_direct_hook_child_of_the_runtime_is_admitted() {
        let chain = vec![
            corridor_link(11, "/opt/test/bin/farhelm", &hook_argv()),
            corridor_link(10, "/opt/test/bin/codex", &["codex"]),
        ];
        let emitter = codex_corridor(&chain).expect("a direct hook child must be admitted");
        assert_eq!(emitter.pid, 10, "the emitter is the one Codex image");
        assert_eq!(emitter.start, 10_000);
    }

    /// The vendor shape: `sh -c` directly invoking the hook command. Both
    /// vendors run hook commands through a shell, so refusing this would
    /// refuse production.
    #[farhelm_testtrace::test]
    fn a_shell_trampoline_invoking_only_the_hook_is_admitted() {
        let chain = vec![
            corridor_link(12, "/opt/test/bin/farhelm", &hook_argv()),
            corridor_link(
                11,
                "/bin/sh",
                &[
                    "sh",
                    "-c",
                    "/opt/test/bin/farhelm internal hook --vendor codex",
                ],
            ),
            corridor_link(10, "/opt/test/bin/codex", &["codex"]),
        ];
        let emitter = codex_corridor(&chain).expect("a hook trampoline must be admitted");
        assert_eq!(emitter.pid, 10);
    }

    /// The pane anchor is accepted by position: a wrapper between the
    /// runtime and the pane (here the login shell the pane reports) is
    /// the owned foreground, not an intermediary.
    #[farhelm_testtrace::test]
    fn a_wrapper_above_the_runtime_is_the_pane_anchor_not_an_intermediary() {
        let chain = vec![
            corridor_link(12, "/opt/test/bin/farhelm", &hook_argv()),
            corridor_link(11, "/opt/test/bin/codex", &["codex"]),
            corridor_link(10, "/bin/bash", &["-bash"]),
        ];
        codex_corridor(&chain).expect("the pane anchor must not need trampoline shape");
    }

    /// A reporter that is not the hook invocation cannot report, even with
    /// a clean chain behind it: anything holding the socket credential
    /// could otherwise speak the protocol directly.
    #[farhelm_testtrace::test]
    fn a_reporter_without_hook_shape_is_refused() {
        for argv in [
            vec!["/opt/test/bin/farhelm"],
            vec!["/opt/test/bin/farhelm", "internal"],
            vec!["/opt/test/bin/farhelm", "agent", "instructions"],
            vec!["codex"],
        ] {
            let chain = vec![
                corridor_link(11, "/opt/test/bin/farhelm", &argv),
                corridor_link(10, "/opt/test/bin/codex", &["codex"]),
            ];
            let refusal = codex_corridor(&chain).expect_err("a non-hook reporter must be refused");
            assert!(
                refusal.contains("supported hook invocation"),
                "the diagnostic must name the missing hook shape: {refusal}"
            );
        }
    }

    /// Missing reporter argv is missing evidence, not a pass: the shape
    /// cannot be established without it.
    #[farhelm_testtrace::test]
    fn a_reporter_without_readable_argv_is_refused() {
        let chain = vec![
            corridor_link_no_argv(11, "/opt/test/bin/farhelm"),
            corridor_link(10, "/opt/test/bin/codex", &["codex"]),
        ];
        let refusal = codex_corridor(&chain).expect_err("missing reporter argv must be refused");
        assert!(refusal.contains("supported hook invocation"), "{refusal}");
    }

    /// An interactive shell between reporter and runtime is unclassified:
    /// it may run anything, so the corridor cannot treat it as a
    /// transparent trampoline.
    #[farhelm_testtrace::test]
    fn an_interactive_shell_intermediary_is_refused() {
        let chain = vec![
            corridor_link(13, "/opt/test/bin/farhelm", &hook_argv()),
            corridor_link(12, "/bin/bash", &["bash"]),
            corridor_link(
                11,
                "/bin/sh",
                &["sh", "-c", "farhelm internal hook --vendor codex"],
            ),
            corridor_link(10, "/opt/test/bin/codex", &["codex"]),
        ];
        // The inner trampoline is well-formed; the interactive shell above
        // it is not, and one unclassified link refuses the whole chain.
        let refusal = codex_corridor(&chain).expect_err("an interactive shell must be refused");
        assert!(refusal.contains("unclassified intermediary"), "{refusal}");
    }

    /// A shell running a script is not a trampoline even when the script
    /// eventually runs the hook: the corridor sees the intermediary's own
    /// shape (`sh script`), not what the script does later.
    #[farhelm_testtrace::test]
    fn a_script_running_shell_intermediary_is_refused() {
        let chain = vec![
            corridor_link(12, "/opt/test/bin/farhelm", &hook_argv()),
            corridor_link(11, "/bin/sh", &["sh", "/tmp/reporter.sh"]),
            corridor_link(10, "/opt/test/bin/codex", &["codex"]),
        ];
        let refusal = codex_corridor(&chain).expect_err("a script intermediary must be refused");
        assert!(refusal.contains("unclassified intermediary"), "{refusal}");
    }

    /// A chained `-c` command mentions the hook without being only the
    /// hook: substring presence must not pass.
    #[farhelm_testtrace::test]
    fn a_chained_shell_command_mentioning_the_hook_is_refused() {
        let chain = vec![
            corridor_link(12, "/opt/test/bin/farhelm", &hook_argv()),
            corridor_link(
                11,
                "/bin/sh",
                &[
                    "sh",
                    "-c",
                    "echo ready; farhelm internal hook --vendor codex",
                ],
            ),
            corridor_link(10, "/opt/test/bin/codex", &["codex"]),
        ];
        let refusal =
            codex_corridor(&chain).expect_err("a chained command must not pass as a trampoline");
        assert!(refusal.contains("unclassified intermediary"), "{refusal}");
    }

    /// A hook-first `-c` command with a redirection and a second command
    /// still runs other shell syntax after the hook: the word splitter
    /// sees the hook in the first three words and nothing wrong, but the
    /// shell executes the redirection and the chained command too. The
    /// trampoline must be exactly one simple command.
    #[farhelm_testtrace::test]
    fn a_hook_first_chained_shell_command_is_refused() {
        let chain = vec![
            corridor_link(12, "/opt/test/bin/farhelm", &hook_argv()),
            corridor_link(
                11,
                "/bin/sh",
                &[
                    "sh",
                    "-c",
                    "/opt/test/bin/farhelm internal hook --vendor codex < /tmp/child-report.json; printf done",
                ],
            ),
            corridor_link(10, "/opt/test/bin/codex", &["codex"]),
        ];
        let refusal = codex_corridor(&chain)
            .expect_err("a hook-first chained command must not pass as a trampoline");
        assert!(refusal.contains("unclassified intermediary"), "{refusal}");
    }

    /// A redirection alone is executable shell syntax beyond the hook:
    /// the same single command with its stdin replaced is not the
    /// supported simple command.
    #[farhelm_testtrace::test]
    fn a_hook_command_with_a_redirection_is_refused() {
        let chain = vec![
            corridor_link(12, "/opt/test/bin/farhelm", &hook_argv()),
            corridor_link(
                11,
                "/bin/sh",
                &[
                    "sh",
                    "-c",
                    "/opt/test/bin/farhelm internal hook --vendor codex < /tmp/child-report.json",
                ],
            ),
            corridor_link(10, "/opt/test/bin/codex", &["codex"]),
        ];
        let refusal = codex_corridor(&chain)
            .expect_err("a redirected hook command must not pass as a trampoline");
        assert!(refusal.contains("unclassified intermediary"), "{refusal}");
    }

    /// A substitution inside `-c` runs a second command to build the
    /// hook's surroundings. Word splitting hides it inside one word; the
    /// syntax scan must still see the `$`.
    #[farhelm_testtrace::test]
    fn a_hook_command_with_a_substitution_is_refused() {
        for command in [
            "/opt/test/bin/farhelm internal hook --vendor $(printf codex)",
            "/opt/test/bin/farhelm internal hook --vendor `printf codex`",
            "/opt/test/bin/farhelm internal hook --vendor \"$(printf codex)\"",
            "/opt/test/bin/farhelm internal hook --vendor codex | tee /tmp/hook.log",
        ] {
            let chain = vec![
                corridor_link(12, "/opt/test/bin/farhelm", &hook_argv()),
                corridor_link(11, "/bin/sh", &["sh", "-c", command]),
                corridor_link(10, "/opt/test/bin/codex", &["codex"]),
            ];
            let refusal = codex_corridor(&chain)
                .expect_err("a hook command with extra shell syntax must be refused: {command}");
            assert!(refusal.contains("unclassified intermediary"), "{refusal}");
        }
    }

    /// Metacharacters inside a quoted executable path are literal path
    /// characters, not shell syntax: a trampoline whose quoted path
    /// contains `;` must still be admitted, or installs under
    /// punctuation-bearing directories would break. Both quote styles
    /// protect; only double quotes still expand `$` and backquotes.
    #[farhelm_testtrace::test]
    fn a_trampoline_with_a_quoted_path_holding_metacharacters_is_admitted() {
        for command in [
            "\"/opt/test/bin with;weird/farhelm\" internal hook --vendor codex",
            "'/opt/test/bin$weird/farhelm' internal hook --vendor codex",
        ] {
            let chain = vec![
                corridor_link(12, "/opt/test/bin/farhelm", &hook_argv()),
                corridor_link(11, "/bin/sh", &["sh", "-c", command]),
                corridor_link(10, "/opt/test/bin/codex", &["codex"]),
            ];
            let emitter = codex_corridor(&chain)
                .expect("a quoted-path trampoline must be admitted: {command}");
            assert_eq!(emitter.pid, 10);
        }
    }

    /// Another harness between reporter and runtime refuses with its own
    /// message, so the diagnostic names the mechanism.
    #[farhelm_testtrace::test]
    fn another_runtime_between_reporter_and_foreground_is_refused() {
        for (exe, message) in [
            ("/opt/test/bin/claude", "another session-hosting runtime"),
            ("/usr/local/bin/node", "unclassified intermediary"),
        ] {
            let chain = vec![
                corridor_link(12, "/opt/test/bin/farhelm", &hook_argv()),
                corridor_link(11, exe, &[exe, "--run", "harness"]),
                corridor_link(10, "/opt/test/bin/codex", &["codex"]),
            ];
            let refusal =
                codex_corridor(&chain).expect_err("a foreign intermediary must be refused");
            assert!(refusal.contains(message), "{refusal}");
        }
    }

    /// The pre-existing nested rule, preserved: two Codex images refuse no
    /// matter how clean the rest of the chain is.
    #[farhelm_testtrace::test]
    fn a_second_codex_image_still_refuses() {
        let chain = vec![
            corridor_link(13, "/opt/test/bin/farhelm", &hook_argv()),
            corridor_link(12, "/opt/test/bin/codex", &["codex"]),
            corridor_link(11, "/opt/test/bin/codex", &["codex"]),
        ];
        let refusal = codex_corridor(&chain).expect_err("nested Codex must be refused");
        assert!(refusal.contains("nested Codex"), "{refusal}");
    }

    /// No Codex image at all still refuses: the corridor never admits on
    /// ancestry shape alone.
    #[farhelm_testtrace::test]
    fn a_chain_without_any_codex_image_is_refused() {
        let chain = vec![
            corridor_link(12, "/opt/test/bin/farhelm", &hook_argv()),
            corridor_link(
                11,
                "/bin/sh",
                &["sh", "-c", "farhelm internal hook --vendor codex"],
            ),
            corridor_link(10, "/bin/bash", &["-bash"]),
        ];
        let refusal = codex_corridor(&chain).expect_err("a chain without Codex must be refused");
        assert!(
            refusal.contains("no attributable Codex executable"),
            "{refusal}"
        );
    }

    /// Missing intermediary argv cannot match the trampoline shape, so it
    /// refuses rather than passing on image alone.
    #[farhelm_testtrace::test]
    fn an_intermediary_without_readable_argv_is_refused() {
        let chain = vec![
            corridor_link(12, "/opt/test/bin/farhelm", &hook_argv()),
            corridor_link_no_argv(11, "/bin/sh"),
            corridor_link(10, "/opt/test/bin/codex", &["codex"]),
        ];
        let refusal =
            codex_corridor(&chain).expect_err("an unreadable intermediary must be refused");
        assert!(refusal.contains("unclassified intermediary"), "{refusal}");
    }

    /// The installed hook command for OMP reports, as the kernel captures
    /// it on the reporter.
    fn omp_hook_argv() -> Vec<&'static str> {
        vec![
            "/opt/test/bin/farhelm",
            "internal",
            "hook",
            "--vendor",
            "omp",
        ]
    }

    /// The installed-shape runtime link: the Bun image running the bundle
    /// entry at the observed install path, with TUI argv behind it.
    fn omp_runtime_link(pid: u32, tail: &[&str]) -> ChainLink {
        let mut argv = vec![
            "bun",
            "/opt/omp/node_modules/@oh-my-pi/pi-coding-agent/dist/cli.js",
        ];
        argv.extend(tail.iter().copied());
        corridor_link(pid, "/opt/bun/bin/bun", &argv)
    }

    /// The installed `omp` launch admits a direct hook child of the
    /// Bun-executed bundle: interpreter plus canonical entry plus TUI
    /// grammar, with the pane anchor accepted by position.
    ///
    /// Why this test matters: it is the primary supported shape — the one
    /// production launches take — so the corridor must keep admitting it
    /// while every refusal below stays closed.
    #[farhelm_testtrace::test]
    fn an_installed_omp_runtime_with_a_direct_hook_child_is_admitted() {
        let chain = vec![
            corridor_link(12, "/opt/test/bin/farhelm", &omp_hook_argv()),
            omp_runtime_link(11, &[]),
            corridor_link(10, "/bin/bash", &["-bash"]),
        ];
        let emitter = omp_corridor(&chain, &crate::agent_kind::omp::OmpLaunchProgram::Omp)
            .expect("the installed shape must be admitted");
        assert_eq!(emitter.pid, 11, "the emitter is the Bun runtime");
    }

    /// The pane anchor may itself be the runtime: a pane that exec'd into
    /// the launch has no wrapper link left, and position must not cost it
    /// the emitter role.
    #[farhelm_testtrace::test]
    fn a_pane_execd_into_the_runtime_is_still_the_emitter() {
        let chain = vec![
            corridor_link(12, "/opt/test/bin/farhelm", &omp_hook_argv()),
            omp_runtime_link(11, &["--resume", "conv.jsonl"]),
        ];
        let emitter = omp_corridor(&chain, &crate::agent_kind::omp::OmpLaunchProgram::Omp)
            .expect("an exec'd pane runtime must be admitted");
        assert_eq!(emitter.pid, 11);
    }

    /// The compiled target admits with the same TUI grammar: a native
    /// `omp` image whose argv stays interactive.
    #[farhelm_testtrace::test]
    fn a_compiled_omp_target_with_tui_argv_is_admitted() {
        let chain = vec![
            corridor_link(12, "/opt/test/bin/farhelm", &omp_hook_argv()),
            corridor_link(11, "/opt/test/bin/omp", &["omp", "--resume", "conv.jsonl"]),
            corridor_link(10, "/bin/bash", &["-bash"]),
        ];
        let emitter = omp_corridor(&chain, &crate::agent_kind::omp::OmpLaunchProgram::Omp)
            .expect("the compiled shape must be admitted");
        assert_eq!(emitter.pid, 11);
    }

    /// Two OMP runtimes refuse: a nested interactive child passes the
    /// asset gate on its own context, so only process attribution can
    /// refuse it — this is the central regression the corridor pins.
    #[farhelm_testtrace::test]
    fn a_nested_omp_runtime_is_refused() {
        let chain = vec![
            corridor_link(13, "/opt/test/bin/farhelm", &omp_hook_argv()),
            omp_runtime_link(12, &[]),
            omp_runtime_link(11, &[]),
        ];
        let refusal = omp_corridor(&chain, &crate::agent_kind::omp::OmpLaunchProgram::Omp)
            .expect_err("nested OMP must be refused");
        assert!(refusal.contains("nested OMP"), "{refusal}");
    }

    /// Node-executed OMP refuses with its own diagnostic: execution and
    /// lifecycle parity for Node is unverified, so the shape fails closed
    /// rather than riding the `.js` suffix into the Bun rule.
    #[farhelm_testtrace::test]
    fn a_node_executed_omp_entry_is_refused() {
        let chain = vec![
            corridor_link(12, "/opt/test/bin/farhelm", &omp_hook_argv()),
            corridor_link(
                11,
                "/opt/node/bin/node",
                &[
                    "node",
                    "/opt/omp/node_modules/@oh-my-pi/pi-coding-agent/dist/cli.js",
                ],
            ),
            corridor_link(10, "/bin/bash", &["-bash"]),
        ];
        let refusal = omp_corridor(&chain, &crate::agent_kind::omp::OmpLaunchProgram::Omp)
            .expect_err("Node execution must be refused");
        assert!(refusal.contains("Node-executed OMP"), "{refusal}");
    }

    /// A direct Node launch program refuses before any link is examined:
    /// there is no descriptor in which it could be legitimate.
    #[farhelm_testtrace::test]
    fn a_node_launch_program_is_refused_upfront() {
        let chain = vec![
            corridor_link(12, "/opt/test/bin/farhelm", &omp_hook_argv()),
            omp_runtime_link(11, &[]),
        ];
        let refusal = omp_corridor(&chain, &crate::agent_kind::omp::OmpLaunchProgram::Node)
            .expect_err("a Node launch program must be refused");
        assert!(refusal.contains("Node-executed OMP"), "{refusal}");
    }

    /// An unknown launch program refuses: the live chain must agree with
    /// how the session was launched, not merely look like some OMP shape.
    #[farhelm_testtrace::test]
    fn an_unknown_launch_program_is_refused() {
        let chain = vec![
            corridor_link(12, "/opt/test/bin/farhelm", &omp_hook_argv()),
            omp_runtime_link(11, &[]),
        ];
        let refusal = omp_corridor(&chain, &crate::agent_kind::omp::OmpLaunchProgram::Unknown)
            .expect_err("an unknown launch program must be refused");
        assert!(refusal.contains("not a supported runtime"), "{refusal}");
    }

    /// A same-named entry elsewhere is not the selected bundle: suffix
    /// matching keeps a decoy `dist/cli.js` out of the descriptor.
    #[farhelm_testtrace::test]
    fn a_same_named_entry_outside_the_bundle_is_refused() {
        let chain = vec![
            corridor_link(12, "/opt/test/bin/farhelm", &omp_hook_argv()),
            corridor_link(11, "/opt/bun/bin/bun", &["bun", "/tmp/dist/cli.js"]),
            corridor_link(10, "/bin/bash", &["-bash"]),
        ];
        let refusal = omp_corridor(&chain, &crate::agent_kind::omp::OmpLaunchProgram::Omp)
            .expect_err("a decoy entry must be refused");
        assert!(refusal.contains("no attributable OMP runtime"), "{refusal}");
    }

    /// A Bun runtime without readable argv is missing evidence, not an
    /// emitter: the descriptor binds interpreter AND entry point, and an
    /// entry point that cannot be read cannot be bound.
    #[farhelm_testtrace::test]
    fn a_bun_runtime_without_readable_argv_is_refused() {
        let chain = vec![
            corridor_link(12, "/opt/test/bin/farhelm", &omp_hook_argv()),
            corridor_link_no_argv(11, "/opt/bun/bin/bun"),
            corridor_link(10, "/bin/bash", &["-bash"]),
        ];
        let refusal = omp_corridor(&chain, &crate::agent_kind::omp::OmpLaunchProgram::Omp)
            .expect_err("an argv-less Bun link must be refused");
        assert!(refusal.contains("no attributable OMP runtime"), "{refusal}");
    }

    /// The live runtime argv must still describe an interactive
    /// conversation: a process that exec'd from a TUI launch into `--print`
    /// is no longer the interactive runtime, even though its launch was.
    #[farhelm_testtrace::test]
    fn a_runtime_execd_into_print_shape_is_refused() {
        let chain = vec![
            corridor_link(12, "/opt/test/bin/farhelm", &omp_hook_argv()),
            omp_runtime_link(11, &["-p", "summarize this"]),
            corridor_link(10, "/bin/bash", &["-bash"]),
        ];
        let refusal = omp_corridor(&chain, &crate::agent_kind::omp::OmpLaunchProgram::Omp)
            .expect_err("a print-shaped runtime must be refused");
        assert!(
            refusal.contains("no longer describes an interactive conversation"),
            "{refusal}"
        );
    }

    /// A utility subcommand in the live runtime argv refuses the same way:
    /// the grammar is the launch classifier's, re-read live.
    #[farhelm_testtrace::test]
    fn a_runtime_execd_into_a_utility_command_is_refused() {
        let chain = vec![
            corridor_link(12, "/opt/test/bin/farhelm", &omp_hook_argv()),
            omp_runtime_link(11, &["models"]),
            corridor_link(10, "/bin/bash", &["-bash"]),
        ];
        let refusal = omp_corridor(&chain, &crate::agent_kind::omp::OmpLaunchProgram::Omp)
            .expect_err("a utility-shaped runtime must be refused");
        assert!(
            refusal.contains("no longer describes an interactive conversation"),
            "{refusal}"
        );
    }

    /// A hook trampoline between reporter and runtime admits: the vendor
    /// runs hook commands through a shell, and the corridor must not
    /// refuse production's own shape.
    #[farhelm_testtrace::test]
    fn a_hook_trampoline_below_an_omp_runtime_is_admitted() {
        let chain = vec![
            corridor_link(13, "/opt/test/bin/farhelm", &omp_hook_argv()),
            corridor_link(
                12,
                "/bin/sh",
                &["sh", "-c", "farhelm internal hook --vendor omp"],
            ),
            omp_runtime_link(11, &[]),
            corridor_link(10, "/bin/bash", &["-bash"]),
        ];
        let emitter = omp_corridor(&chain, &crate::agent_kind::omp::OmpLaunchProgram::Omp)
            .expect("a hook trampoline must be admitted");
        assert_eq!(emitter.pid, 11);
    }

    /// An interactive shell between reporter and runtime refuses: only the
    /// narrow hook trampoline may sit below the runtime.
    #[farhelm_testtrace::test]
    fn an_interactive_shell_below_an_omp_runtime_is_refused() {
        let chain = vec![
            corridor_link(13, "/opt/test/bin/farhelm", &omp_hook_argv()),
            corridor_link(12, "/bin/bash", &["bash"]),
            omp_runtime_link(11, &[]),
        ];
        let refusal = omp_corridor(&chain, &crate::agent_kind::omp::OmpLaunchProgram::Omp)
            .expect_err("an interactive shell must be refused");
        assert!(refusal.contains("unclassified intermediary"), "{refusal}");
    }

    /// A mixed-harness intermediary refuses with its own diagnostic: an
    /// outer OMP with a Pi descendant between runtime and reporter traveled
    /// through a different harness, even with inherited credentials.
    #[farhelm_testtrace::test]
    fn a_mixed_harness_intermediary_is_refused() {
        let chain = vec![
            corridor_link(13, "/opt/test/bin/farhelm", &omp_hook_argv()),
            corridor_link(12, "/opt/test/bin/pi", &["pi", "-p", "hi"]),
            omp_runtime_link(11, &[]),
        ];
        let refusal = omp_corridor(&chain, &crate::agent_kind::omp::OmpLaunchProgram::Omp)
            .expect_err("a mixed-harness intermediary must be refused");
        assert!(
            refusal.contains("another session-hosting runtime"),
            "{refusal}"
        );
    }

    /// A `bun x` launcher above the runtime admits for a Bun launch: the
    /// package manager sits above the managed runtime, never below it.
    #[farhelm_testtrace::test]
    fn a_bun_package_launcher_above_the_runtime_is_admitted() {
        let chain = vec![
            corridor_link(13, "/opt/test/bin/farhelm", &omp_hook_argv()),
            omp_runtime_link(12, &[]),
            corridor_link(
                11,
                "/opt/bun/bin/bun",
                &["bun", "x", "@oh-my-pi/pi-coding-agent"],
            ),
            corridor_link(10, "/bin/bash", &["-bash"]),
        ];
        let emitter = omp_corridor(&chain, &crate::agent_kind::omp::OmpLaunchProgram::Bun)
            .expect("a bun package launcher must be admitted");
        assert_eq!(emitter.pid, 12);
    }

    /// A launcher selecting another package refuses: the exact package
    /// selection is what makes a launcher this launch's, and an unrelated
    /// package above the runtime contradicts it.
    #[farhelm_testtrace::test]
    fn a_bun_launcher_for_another_package_is_refused() {
        let chain = vec![
            corridor_link(13, "/opt/test/bin/farhelm", &omp_hook_argv()),
            omp_runtime_link(12, &[]),
            corridor_link(11, "/opt/bun/bin/bun", &["bun", "x", "some-other-tool"]),
            corridor_link(10, "/bin/bash", &["-bash"]),
        ];
        let refusal = omp_corridor(&chain, &crate::agent_kind::omp::OmpLaunchProgram::Bun)
            .expect_err("a foreign package launcher must be refused");
        assert!(refusal.contains("unclassified intermediary"), "{refusal}");
    }

    /// A launcher between runtime and pane anchor contradicts an
    /// installed-`omp` launch: the installed command runs its runtime
    /// directly under the pane, so a package manager in between is not
    /// this launch's shape. (The pane anchor itself is accepted by
    /// position — the framework trusts the owned pane's own process, and
    /// this test keeps a real anchor below the launcher to pin the
    /// middle-link rule rather than the anchor rule.)
    #[farhelm_testtrace::test]
    fn a_launcher_above_an_installed_omp_launch_is_refused() {
        let chain = vec![
            corridor_link(13, "/opt/test/bin/farhelm", &omp_hook_argv()),
            omp_runtime_link(12, &[]),
            corridor_link(
                11,
                "/opt/bun/bin/bun",
                &["bun", "x", "@oh-my-pi/pi-coding-agent"],
            ),
            corridor_link(10, "/bin/bash", &["-bash"]),
        ];
        let refusal = omp_corridor(&chain, &crate::agent_kind::omp::OmpLaunchProgram::Omp)
            .expect_err("a launcher above an installed launch must be refused");
        assert!(refusal.contains("unclassified intermediary"), "{refusal}");
    }

    /// An npm launcher selecting OMP's exact package admits for an npm
    /// launch, with the Bun-resulting runtime below it carrying the proof.
    #[farhelm_testtrace::test]
    fn an_npm_launcher_with_exact_package_selection_is_admitted() {
        let chain = vec![
            corridor_link(13, "/opt/test/bin/farhelm", &omp_hook_argv()),
            omp_runtime_link(12, &[]),
            corridor_link(
                11,
                "/opt/node/bin/node",
                &[
                    "node",
                    "/opt/node/lib/node_modules/npm/bin/npm-cli.js",
                    "exec",
                    "--package=@oh-my-pi/pi-coding-agent",
                    "--",
                    "omp",
                ],
            ),
            corridor_link(10, "/bin/bash", &["-bash"]),
        ];
        let emitter = omp_corridor(&chain, &crate::agent_kind::omp::OmpLaunchProgram::Npm)
            .expect("an exact npm selection must be admitted");
        assert_eq!(emitter.pid, 12);
    }

    /// An npm command string refuses even with an otherwise valid OMP
    /// package selection, boundary, and command: `-c` runs its string
    /// as a script instead of the selected bin. The selection is valid
    /// on purpose — the admitted-selection test above accepts this
    /// chain minus the `-c`, so the refusal isolates the command
    /// string rather than a missing selection.
    #[farhelm_testtrace::test]
    fn an_npm_command_string_is_refused() {
        let chain = vec![
            corridor_link(13, "/opt/test/bin/farhelm", &omp_hook_argv()),
            omp_runtime_link(12, &[]),
            corridor_link(
                11,
                "/opt/node/bin/node",
                &[
                    "node",
                    "/opt/node/lib/node_modules/npm/bin/npm-cli.js",
                    "exec",
                    "--package=@oh-my-pi/pi-coding-agent",
                    "-c",
                    "omp --version",
                    "--",
                    "omp",
                ],
            ),
            corridor_link(10, "/bin/bash", &["-bash"]),
        ];
        let refusal = omp_corridor(&chain, &crate::agent_kind::omp::OmpLaunchProgram::Npm)
            .expect_err("an npm command string must be refused");
        assert!(refusal.contains("unclassified intermediary"), "{refusal}");
    }

    /// The inline command-string spelling refuses too: `--call=...`
    /// is the same script mode as `-c`, and the valid selection,
    /// boundary, and command around it isolate the spelling as the
    /// reason, the way the `-c` test above does.
    #[farhelm_testtrace::test]
    fn an_npm_inline_command_string_is_refused() {
        let chain = vec![
            corridor_link(13, "/opt/test/bin/farhelm", &omp_hook_argv()),
            omp_runtime_link(12, &[]),
            corridor_link(
                11,
                "/opt/node/bin/node",
                &[
                    "node",
                    "/opt/node/lib/node_modules/npm/bin/npm-cli.js",
                    "exec",
                    "--package=@oh-my-pi/pi-coding-agent",
                    "--call=omp --version",
                    "--",
                    "omp",
                ],
            ),
            corridor_link(10, "/bin/bash", &["-bash"]),
        ];
        let refusal = omp_corridor(&chain, &crate::agent_kind::omp::OmpLaunchProgram::Npm)
            .expect_err("an inline npm command string must be refused");
        assert!(refusal.contains("unclassified intermediary"), "{refusal}");
    }

    /// A foreign command after the boundary refuses: the package being
    /// available is not the bin being selected. Paired with the
    /// admitted-selection test, which differs only in the command
    /// token, so the refusal isolates the selection.
    #[farhelm_testtrace::test]
    fn a_foreign_command_after_the_npm_boundary_is_refused() {
        let chain = vec![
            corridor_link(13, "/opt/test/bin/farhelm", &omp_hook_argv()),
            omp_runtime_link(12, &[]),
            corridor_link(
                11,
                "/opt/node/bin/node",
                &[
                    "node",
                    "/opt/node/lib/node_modules/npm/bin/npm-cli.js",
                    "exec",
                    "--package=@oh-my-pi/pi-coding-agent",
                    "--",
                    "some-other-command",
                ],
            ),
            corridor_link(10, "/bin/bash", &["-bash"]),
        ];
        let refusal = omp_corridor(&chain, &crate::agent_kind::omp::OmpLaunchProgram::Npm)
            .expect_err("a foreign command after the boundary must be refused");
        assert!(refusal.contains("unclassified intermediary"), "{refusal}");
    }

    /// npx's bare positional package form admits: the first token names
    /// OMP's package and npx runs its bin, with the Bun-resulting
    /// runtime below carrying the proof as in the npm case.
    #[farhelm_testtrace::test]
    fn an_npx_positional_package_is_admitted() {
        let chain = vec![
            corridor_link(13, "/opt/test/bin/farhelm", &omp_hook_argv()),
            omp_runtime_link(12, &[]),
            corridor_link(
                11,
                "/opt/node/bin/node",
                &[
                    "node",
                    "/opt/node/lib/node_modules/npm/bin/npx-cli.js",
                    "@oh-my-pi/pi-coding-agent",
                ],
            ),
            corridor_link(10, "/bin/bash", &["-bash"]),
        ];
        let emitter = omp_corridor(&chain, &crate::agent_kind::omp::OmpLaunchProgram::Npm)
            .expect("a positional npx package selection must be admitted");
        assert_eq!(emitter.pid, 12);
    }

    /// npx's option form admits with the same option grammar as npm's:
    /// exact `--package` selectors in mixed joined and separated
    /// spellings, repeats allowed, then the `--` boundary and OMP's
    /// bin. This pins the third accepted form and the selector
    /// multiplicity the grammar documents — the `selected` flag the
    /// refusal test below checks is what these repeats set.
    #[farhelm_testtrace::test]
    fn an_npx_option_form_with_repeated_selectors_is_admitted() {
        let chain = vec![
            corridor_link(13, "/opt/test/bin/farhelm", &omp_hook_argv()),
            omp_runtime_link(12, &[]),
            corridor_link(
                11,
                "/opt/node/bin/node",
                &[
                    "node",
                    "/opt/node/lib/node_modules/npm/bin/npx-cli.js",
                    "--package=@oh-my-pi/pi-coding-agent",
                    "--package",
                    "@oh-my-pi/pi-coding-agent",
                    "--",
                    "omp",
                ],
            ),
            corridor_link(10, "/bin/bash", &["-bash"]),
        ];
        let emitter = omp_corridor(&chain, &crate::agent_kind::omp::OmpLaunchProgram::Npm)
            .expect("an npx option-form selection must be admitted");
        assert_eq!(emitter.pid, 12);
    }

    /// A boundary with no package selector refuses: the option grammar
    /// requires at least one exact selector, so a bare `-- omp` names
    /// no package to run the bin from. The admitted option-form test
    /// above differs only in carrying selectors, isolating the
    /// lower bound of the documented multiplicity.
    #[farhelm_testtrace::test]
    fn an_npm_boundary_without_a_package_selection_is_refused() {
        let chain = vec![
            corridor_link(13, "/opt/test/bin/farhelm", &omp_hook_argv()),
            omp_runtime_link(12, &[]),
            corridor_link(
                11,
                "/opt/node/bin/node",
                &[
                    "node",
                    "/opt/node/lib/node_modules/npm/bin/npm-cli.js",
                    "exec",
                    "--",
                    "omp",
                ],
            ),
            corridor_link(10, "/bin/bash", &["-bash"]),
        ];
        let refusal = omp_corridor(&chain, &crate::agent_kind::omp::OmpLaunchProgram::Npm)
            .expect_err("a selector-less boundary must be refused");
        assert!(refusal.contains("unclassified intermediary"), "{refusal}");
    }

    /// A transparent shell trampoline exec'ing the launch's runtime admits:
    /// the wrapper disappears into the runtime it names.
    #[farhelm_testtrace::test]
    fn a_shell_trampoline_execing_the_runtime_is_admitted() {
        let chain = vec![
            corridor_link(13, "/opt/test/bin/farhelm", &omp_hook_argv()),
            omp_runtime_link(12, &[]),
            corridor_link(11, "/bin/sh", &["sh", "-c", "exec omp --resume conv.jsonl"]),
            corridor_link(10, "/bin/bash", &["-bash"]),
        ];
        let emitter = omp_corridor(&chain, &crate::agent_kind::omp::OmpLaunchProgram::Omp)
            .expect("a transparent trampoline must be admitted");
        assert_eq!(emitter.pid, 12);
    }

    /// A shell trampoline exec'ing another program is not this launch's
    /// transparency: the target must name the launch's own runtime.
    #[farhelm_testtrace::test]
    fn a_shell_trampoline_execing_another_program_is_refused() {
        let chain = vec![
            corridor_link(13, "/opt/test/bin/farhelm", &omp_hook_argv()),
            omp_runtime_link(12, &[]),
            corridor_link(11, "/bin/sh", &["sh", "-c", "exec some-wrapper"]),
            corridor_link(10, "/bin/bash", &["-bash"]),
        ];
        let refusal = omp_corridor(&chain, &crate::agent_kind::omp::OmpLaunchProgram::Omp)
            .expect_err("a foreign trampoline target must be refused");
        assert!(refusal.contains("unclassified intermediary"), "{refusal}");
    }

    /// A chained trampoline command mentions the runtime without being
    /// only the runtime: substring presence must not pass.
    #[farhelm_testtrace::test]
    fn a_chained_trampoline_mentioning_the_runtime_is_refused() {
        let chain = vec![
            corridor_link(13, "/opt/test/bin/farhelm", &omp_hook_argv()),
            omp_runtime_link(12, &[]),
            corridor_link(11, "/bin/sh", &["sh", "-c", "echo ready; exec omp"]),
            corridor_link(10, "/bin/bash", &["-bash"]),
        ];
        let refusal = omp_corridor(&chain, &crate::agent_kind::omp::OmpLaunchProgram::Omp)
            .expect_err("a chained trampoline must be refused");
        assert!(refusal.contains("unclassified intermediary"), "{refusal}");
    }

    /// An entry spelling through a symlink resolves to the bundle: the
    /// installed `omp` command is a symlink, and the kernel hands Bun
    /// the launched spelling rather than the resolved bundle path —
    /// observed live as `bun <home>/.bun/bin/omp ...`, which the
    /// raw suffix alone would refuse. A dangling spelling resolves to
    /// nothing and refuses through the raw fallback.
    ///
    /// Why this test matters: without resolution, every production
    /// launch fails the descriptor while synthetic direct-entry chains
    /// pass — the suite would prove a shape production never takes.
    #[farhelm_testtrace::test]
    fn an_entry_spelling_through_a_symlink_resolves_to_the_bundle() {
        let scratch = farhelm_teststate::tempdir().expect("fixture directory");
        let entry = scratch
            .path()
            .join("node_modules/@oh-my-pi/pi-coding-agent/dist/cli.js");
        std::fs::create_dir_all(entry.parent().expect("bundle parent")).expect("bundle layout");
        std::fs::write(&entry, b"fixture entry").expect("entry file");
        let shim = scratch.path().join("omp-shim");
        std::os::unix::fs::symlink(&entry, &shim).expect("entry symlink");
        let me = std::process::id();
        let resolved = resolved_omp_entry(me, &os_bytes(&shim)).expect("the shim resolves");
        assert!(
            is_omp_bundle_entry(&resolved),
            "the canonical path names the bundle: {resolved:?}"
        );
        assert!(
            resolved_omp_entry(me, b"/nonexistent-omp-entry-xyz").is_none(),
            "a dangling spelling resolves to nothing"
        );
        let shim_arg = shim.to_str().expect("UTF-8 fixture path").to_owned();
        let chain = vec![
            corridor_link(12, "/opt/test/bin/farhelm", &omp_hook_argv()),
            corridor_link(11, "/opt/bun/bin/bun", &["bun", shim_arg.as_str()]),
            corridor_link(10, "/bin/bash", &["-bash"]),
        ];
        let emitter = omp_corridor(&chain, &crate::agent_kind::omp::OmpLaunchProgram::Omp)
            .expect("the symlinked entry admits");
        assert_eq!(emitter.pid, 11);
    }

    /// A non-hook reporter refuses even with a clean chain behind it: the
    /// corridor never admits on ancestry shape alone.
    #[farhelm_testtrace::test]
    fn an_omp_reporter_without_hook_shape_is_refused() {
        let chain = vec![
            corridor_link(
                12,
                "/opt/test/bin/farhelm",
                &["farhelm", "agent", "instructions"],
            ),
            omp_runtime_link(11, &[]),
        ];
        let refusal = omp_corridor(&chain, &crate::agent_kind::omp::OmpLaunchProgram::Omp)
            .expect_err("a non-hook reporter must be refused");
        assert!(refusal.contains("supported hook invocation"), "{refusal}");
    }

    /// The Goose helper invocation, as the kernel captures it on the
    /// reporter: any `argv[0]` plus the exact `internal goose-hook`
    /// words.
    fn goose_hook_argv() -> Vec<&'static str> {
        vec!["farhelm", "internal", "goose-hook"]
    }

    /// One native `goose` image link running a session, with `tail`
    /// appended after the `session` word.
    fn goose_runtime_link(pid: u32, tail: &[&str]) -> ChainLink {
        let mut argv = vec!["goose", "session"];
        argv.extend(tail.iter().copied());
        corridor_link(pid, "/usr/local/bin/goose", &argv)
    }

    /// The exact MCP trampoline command: the persisted declaration's
    /// post-`sh -c` words, byte for byte.
    fn goose_mcp_command() -> &'static str {
        "exec \"${FARHELM_GOOSE_REPORTER_EXE:-farhelm}\" internal goose-hook"
    }

    /// A native `goose` runtime with a direct helper child is the
    /// primary supported shape — the one production launches take, in
    /// which the extension manager's `sh -c` execs the helper away and
    /// no trampoline link survives — so the corridor must keep
    /// admitting it while every refusal below stays closed.
    ///
    /// Why this test matters: it is the shape the live admission
    /// tests exercise, pinned here without processes.
    #[farhelm_testtrace::test]
    fn a_native_goose_runtime_with_a_direct_helper_child_is_admitted() {
        let chain = vec![
            corridor_link(12, "/opt/test/bin/farhelm", &goose_hook_argv()),
            goose_runtime_link(11, &[]),
            corridor_link(10, "/bin/bash", &["-bash"]),
        ];
        let emitter = goose_corridor(&chain, &crate::agent_kind::goose::GooseLaunchProgram::Goose)
            .expect("the installed shape must be admitted");
        assert_eq!(emitter.pid, 11, "the emitter is the goose runtime");
    }

    /// The pane anchor may itself be the runtime: a pane that exec'd
    /// into the launch has no wrapper link left, and position must not
    /// cost it the emitter role.
    #[farhelm_testtrace::test]
    fn a_pane_execd_into_the_goose_runtime_is_still_the_emitter() {
        let chain = vec![
            corridor_link(12, "/opt/test/bin/farhelm", &goose_hook_argv()),
            goose_runtime_link(11, &["--resume", "--session-id", "parent-1"]),
        ];
        let emitter = goose_corridor(&chain, &crate::agent_kind::goose::GooseLaunchProgram::Goose)
            .expect("an exec'd pane runtime must be admitted");
        assert_eq!(emitter.pid, 11);
    }

    /// Resume, fork, and edit argv still attribute: the corridor needs
    /// "recognized" only, because every session shape emits reports
    /// worth attributing — the launch-time resume bit is not
    /// re-decided here.
    #[farhelm_testtrace::test]
    fn session_shaped_argv_variants_still_attribute() {
        for tail in [
            vec!["--resume", "--session-id", "parent-1"],
            vec!["--fork", "--session-id", "parent-1"],
            vec!["--edit", "--session-id", "parent-1"],
            vec![
                "--with-extension",
                "farhelm-reporter:sh -c 'exec farhelm internal goose-hook'",
            ],
        ] {
            let chain = vec![
                corridor_link(12, "/opt/test/bin/farhelm", &goose_hook_argv()),
                goose_runtime_link(11, &tail),
            ];
            goose_corridor(&chain, &crate::agent_kind::goose::GooseLaunchProgram::Goose)
                .unwrap_or_else(|error| panic!("{tail:?} must attribute: {error}"));
        }
    }

    /// A shell launch program admits the exec'd-away wrapper: no link
    /// survives a shell that already exec'd into the runtime, so the
    /// chain looks exactly like a direct launch — and the provenance
    /// saying `shell` must not void it. This is the manual-declaration
    /// shape: injection skipped, reporter declared by hand.
    #[farhelm_testtrace::test]
    fn a_shell_launch_program_with_an_execd_away_wrapper_is_admitted() {
        let chain = vec![
            corridor_link(12, "/opt/test/bin/farhelm", &goose_hook_argv()),
            goose_runtime_link(11, &[]),
        ];
        let emitter = goose_corridor(&chain, &crate::agent_kind::goose::GooseLaunchProgram::Shell)
            .expect("an exec'd-away shell wrapper must be admitted");
        assert_eq!(emitter.pid, 11);
    }

    /// A surviving shell trampoline above the runtime admits under
    /// both programs: `sh -c` exec'ing exactly the runtime is this
    /// launch's transparency either way.
    #[farhelm_testtrace::test]
    fn a_surviving_shell_trampoline_above_the_runtime_is_admitted() {
        for program in [
            crate::agent_kind::goose::GooseLaunchProgram::Goose,
            crate::agent_kind::goose::GooseLaunchProgram::Shell,
        ] {
            let chain = vec![
                corridor_link(13, "/opt/test/bin/farhelm", &goose_hook_argv()),
                goose_runtime_link(12, &[]),
                corridor_link(11, "/bin/sh", &["sh", "-c", "exec goose session"]),
                corridor_link(10, "/bin/bash", &["-bash"]),
            ];
            let emitter = goose_corridor(&chain, &program)
                .expect("a surviving exec trampoline must be admitted");
            assert_eq!(emitter.pid, 12);
        }
    }

    /// The exact MCP trampoline below the runtime admits: a shell
    /// whose `-c` command shell-splits to precisely the persisted
    /// declaration's words, for shells that keep the link instead of
    /// exec'ing it away.
    #[farhelm_testtrace::test]
    fn an_exact_mcp_trampoline_below_the_runtime_is_admitted() {
        let chain = vec![
            corridor_link(13, "/opt/test/bin/farhelm", &goose_hook_argv()),
            corridor_link(12, "/bin/sh", &["sh", "-c", goose_mcp_command()]),
            goose_runtime_link(11, &[]),
        ];
        let emitter = goose_corridor(&chain, &crate::agent_kind::goose::GooseLaunchProgram::Goose)
            .expect("the exact MCP trampoline must be admitted");
        assert_eq!(emitter.pid, 11);
    }

    /// The MCP trampoline matches by exact words, not by resemblance:
    /// a resolved executable, extra words, different words, and a
    /// non-shell image all refuse. Each deviation gets its own chain
    /// so a failure names the deviation, not a row in a loop.
    #[farhelm_testtrace::test]
    fn mcp_trampoline_deviations_are_refused() {
        let chains = [
            (
                "a resolved exe",
                corridor_link(
                    12,
                    "/bin/sh",
                    &["sh", "-c", "exec /opt/test/bin/farhelm internal goose-hook"],
                ),
            ),
            (
                "extra words",
                corridor_link(
                    12,
                    "/bin/sh",
                    &[
                        ("sh"),
                        ("-c"),
                        ("exec \"${FARHELM_GOOSE_REPORTER_EXE:-farhelm}\" internal goose-hook --extra"),
                    ],
                ),
            ),
            (
                "different words",
                corridor_link(12, "/bin/sh", &["sh", "-c", "exec farhelm internal hook"]),
            ),
            (
                "a chained command",
                corridor_link(
                    12,
                    "/bin/sh",
                    &[
                        ("sh"),
                        ("-c"),
                        ("exec \"${FARHELM_GOOSE_REPORTER_EXE:-farhelm}\" internal goose-hook; echo hi"),
                    ],
                ),
            ),
            (
                "a non-shell image",
                corridor_link(12, "/usr/bin/sleep", &["sh", "-c", goose_mcp_command()]),
            ),
        ];
        for (label, trampoline) in chains {
            let chain = vec![
                corridor_link(13, "/opt/test/bin/farhelm", &goose_hook_argv()),
                trampoline,
                goose_runtime_link(11, &[]),
            ];
            let refusal =
                goose_corridor(&chain, &crate::agent_kind::goose::GooseLaunchProgram::Goose)
                    .expect_err(&format!("{label} must be refused"));
            assert!(
                refusal.contains("not the MCP trampoline"),
                "{label}: {refusal}"
            );
        }
    }

    /// A non-trampoline process below the runtime refuses: only the
    /// narrow MCP shape may sit between the reporter and the runtime.
    #[farhelm_testtrace::test]
    fn a_non_trampoline_below_the_runtime_is_refused() {
        let chain = vec![
            corridor_link(13, "/opt/test/bin/farhelm", &goose_hook_argv()),
            corridor_link(12, "/usr/bin/sleep", &["sleep", "25"]),
            goose_runtime_link(11, &[]),
        ];
        let refusal = goose_corridor(&chain, &crate::agent_kind::goose::GooseLaunchProgram::Goose)
            .expect_err("a non-trampoline below the runtime must be refused");
        assert!(refusal.contains("not the MCP trampoline"), "{refusal}");
    }

    /// Two live `goose` images refuse: a nested session passes its own
    /// checks on its own context, so only the duplicate-emitter rule
    /// can refuse it — this is the central regression the corridor
    /// pins.
    #[farhelm_testtrace::test]
    fn a_nested_goose_runtime_is_refused() {
        let chain = vec![
            corridor_link(13, "/opt/test/bin/farhelm", &goose_hook_argv()),
            goose_runtime_link(12, &[]),
            goose_runtime_link(11, &[]),
        ];
        let refusal = goose_corridor(&chain, &crate::agent_kind::goose::GooseLaunchProgram::Goose)
            .expect_err("nested goose must be refused");
        assert!(refusal.contains("two live Goose runtimes"), "{refusal}");
    }

    /// A mixed-harness runtime above the emitter refuses with the
    /// session-hosting diagnostic rather than a generic intermediary:
    /// the shape is recognized, and recognized as another session.
    #[farhelm_testtrace::test]
    fn a_mixed_harness_above_the_runtime_is_refused() {
        for (exe, argv) in [
            ("/opt/test/bin/omp", vec!["omp", "--resume", "conv.jsonl"]),
            ("/usr/local/bin/pi", vec!["pi", "--model", "x"]),
        ] {
            let chain = vec![
                corridor_link(13, "/opt/test/bin/farhelm", &goose_hook_argv()),
                goose_runtime_link(12, &[]),
                corridor_link(11, exe, &argv),
                corridor_link(10, "/bin/bash", &["-bash"]),
            ];
            let refusal =
                goose_corridor(&chain, &crate::agent_kind::goose::GooseLaunchProgram::Goose)
                    .expect_err(&format!("{exe} above the emitter must be refused"));
            assert!(
                refusal.contains("another session-hosting runtime"),
                "{exe}: {refusal}"
            );
        }
    }

    /// An unclassified process above the emitter refuses: anything
    /// that is neither this launch's transparent shell nor a
    /// recognized runtime voids the chain.
    #[farhelm_testtrace::test]
    fn an_unclassified_process_above_the_runtime_is_refused() {
        let chain = vec![
            corridor_link(13, "/opt/test/bin/farhelm", &goose_hook_argv()),
            goose_runtime_link(12, &[]),
            corridor_link(11, "/usr/bin/sleep", &["sleep", "25"]),
            corridor_link(10, "/bin/bash", &["-bash"]),
        ];
        let refusal = goose_corridor(&chain, &crate::agent_kind::goose::GooseLaunchProgram::Goose)
            .expect_err("an unclassified intermediary must be refused");
        assert!(refusal.contains("unclassified process"), "{refusal}");
    }

    /// An unknown launch program refuses before any link is examined:
    /// the chain below would otherwise admit, which is what proves
    /// the refusal is upfront rather than shape-driven.
    #[farhelm_testtrace::test]
    fn an_unknown_goose_launch_program_is_refused_upfront() {
        let chain = vec![
            corridor_link(12, "/opt/test/bin/farhelm", &goose_hook_argv()),
            goose_runtime_link(11, &[]),
        ];
        let refusal = goose_corridor(
            &chain,
            &crate::agent_kind::goose::GooseLaunchProgram::Unknown,
        )
        .expect_err("an unknown launch program must be refused");
        assert!(refusal.contains("not a supported runtime"), "{refusal}");
    }

    /// A non-helper reporter refuses even with a clean chain behind
    /// it: the corridor never admits on ancestry shape alone. The
    /// `internal hook` spelling (other harnesses' helper) is included
    /// because sharing it would cross-wire vendors.
    #[farhelm_testtrace::test]
    fn a_non_helper_reporter_is_refused() {
        for argv in [
            vec!["farhelm", "agent", "instructions"],
            vec!["farhelm", "internal", "hook"],
            vec!["farhelm", "internal", "hook", "--vendor", "goose"],
        ] {
            let chain = vec![
                corridor_link(12, "/opt/test/bin/farhelm", &argv),
                goose_runtime_link(11, &[]),
            ];
            let refusal =
                goose_corridor(&chain, &crate::agent_kind::goose::GooseLaunchProgram::Goose)
                    .expect_err(&format!("{argv:?} must be refused"));
            assert!(
                refusal.contains("not the Goose helper invocation"),
                "{argv:?}: {refusal}"
            );
        }
    }

    /// The reporter rule is exact: trailing words on the helper
    /// invocation refuse, because the helper takes no arguments and a
    /// live peer carrying them is not the invocation this contract
    /// describes.
    #[farhelm_testtrace::test]
    fn a_helper_reporter_with_trailing_words_is_refused() {
        let chain = vec![
            corridor_link(
                12,
                "/opt/test/bin/farhelm",
                &["farhelm", "internal", "goose-hook", "--extra"],
            ),
            goose_runtime_link(11, &[]),
        ];
        let refusal = goose_corridor(&chain, &crate::agent_kind::goose::GooseLaunchProgram::Goose)
            .expect_err("trailing words must be refused");
        assert!(
            refusal.contains("not the Goose helper invocation"),
            "{refusal}"
        );
    }

    /// Missing argv anywhere on the reporter or runtime refuses:
    /// unreadable evidence is missing evidence, never an admission.
    #[farhelm_testtrace::test]
    fn missing_argv_refuses() {
        let chain = vec![
            corridor_link_no_argv(12, "/opt/test/bin/farhelm"),
            goose_runtime_link(11, &[]),
        ];
        let refusal = goose_corridor(&chain, &crate::agent_kind::goose::GooseLaunchProgram::Goose)
            .expect_err("a missing reporter command line must be refused");
        assert!(refusal.contains("could not be read"), "{refusal}");

        let chain = vec![
            corridor_link(12, "/opt/test/bin/farhelm", &goose_hook_argv()),
            corridor_link_no_argv(11, "/usr/local/bin/goose"),
        ];
        let refusal = goose_corridor(&chain, &crate::agent_kind::goose::GooseLaunchProgram::Goose)
            .expect_err("a missing runtime command line must be refused");
        assert!(refusal.contains("could not be read"), "{refusal}");
    }

    /// Non-UTF-8 runtime argv refuses: the grammar decides over decoded
    /// words, and undecodable bytes are missing evidence.
    #[farhelm_testtrace::test]
    fn non_utf8_runtime_argv_is_refused() {
        let chain = vec![
            corridor_link(12, "/opt/test/bin/farhelm", &goose_hook_argv()),
            ChainLink {
                pid: 11,
                ppid: 0,
                start: 11_000,
                exe: b"/usr/local/bin/goose".to_vec(),
                argv: Some(vec![b"goose".to_vec(), b"session".to_vec(), vec![0xff]]),
            },
        ];
        let refusal = goose_corridor(&chain, &crate::agent_kind::goose::GooseLaunchProgram::Goose)
            .expect_err("non-UTF-8 runtime argv must be refused");
        assert!(refusal.contains("not valid UTF-8"), "{refusal}");
    }

    /// A runtime that no longer describes a session refuses even
    /// though its image is still `goose`: the launch-time
    /// classification says how this session started, not what this
    /// process is now. A `--help` tail and a `configure` verb both
    /// leave the session grammar.
    #[farhelm_testtrace::test]
    fn a_runtime_that_left_the_session_grammar_is_refused() {
        for tail in [vec!["--help"], vec!["configure"]] {
            let mut argv = vec!["goose"];
            if tail != ["configure"] {
                argv.push("session");
            }
            argv.extend(tail.iter().copied());
            let chain = vec![
                corridor_link(12, "/opt/test/bin/farhelm", &goose_hook_argv()),
                corridor_link(11, "/usr/local/bin/goose", &argv),
            ];
            let refusal =
                goose_corridor(&chain, &crate::agent_kind::goose::GooseLaunchProgram::Goose)
                    .expect_err(&format!("{argv:?} must be refused"));
            assert!(
                refusal.contains("no longer describes a session"),
                "{argv:?}: {refusal}"
            );
        }
    }

    /// A script named `goose` is not a runtime: its image is the
    /// interpreter's, and the descriptor matches the image — never
    /// `argv[0]`, however session-shaped the words.
    #[farhelm_testtrace::test]
    fn a_script_named_goose_is_not_a_runtime() {
        let chain = vec![
            corridor_link(12, "/opt/test/bin/farhelm", &goose_hook_argv()),
            corridor_link(11, "/bin/sh", &["/usr/local/bin/goose", "session"]),
        ];
        let refusal = goose_corridor(&chain, &crate::agent_kind::goose::GooseLaunchProgram::Goose)
            .expect_err("a script named goose must be refused");
        assert!(refusal.contains("no live Goose runtime"), "{refusal}");
    }

    /// A `goose` image carrying the ` (deleted)` suffix still matches:
    /// a binary replaced while running keeps its identity for the
    /// walk's purposes.
    #[farhelm_testtrace::test]
    fn a_replaced_while_running_goose_image_still_matches() {
        let chain = vec![
            corridor_link(12, "/opt/test/bin/farhelm", &goose_hook_argv()),
            corridor_link(11, "/usr/local/bin/goose (deleted)", &["goose", "session"]),
        ];
        let emitter = goose_corridor(&chain, &crate::agent_kind::goose::GooseLaunchProgram::Goose)
            .expect("a replaced image must still match");
        assert_eq!(emitter.pid, 11);
    }

    /// A shell trampoline exec'ing another program is not this
    /// launch's transparency: only `exec goose` names the runtime the
    /// provenance describes.
    #[farhelm_testtrace::test]
    fn a_goose_shell_trampoline_execing_another_program_is_refused() {
        let chain = vec![
            corridor_link(13, "/opt/test/bin/farhelm", &goose_hook_argv()),
            goose_runtime_link(12, &[]),
            corridor_link(11, "/bin/sh", &["sh", "-c", "exec codex exec"]),
            corridor_link(10, "/bin/bash", &["-bash"]),
        ];
        let refusal = goose_corridor(&chain, &crate::agent_kind::goose::GooseLaunchProgram::Goose)
            .expect_err("a foreign exec target must be refused");
        assert!(refusal.contains("unclassified process"), "{refusal}");
    }

    /// A chained trampoline mentioning the runtime refuses: the `;`
    /// is unquoted shell syntax past the words, so the link is not a
    /// transparent exec no matter which binary it names.
    #[farhelm_testtrace::test]
    fn a_chained_shell_trampoline_is_refused() {
        let chain = vec![
            corridor_link(13, "/opt/test/bin/farhelm", &goose_hook_argv()),
            goose_runtime_link(12, &[]),
            corridor_link(11, "/bin/sh", &["sh", "-c", "exec goose session; echo hi"]),
            corridor_link(10, "/bin/bash", &["-bash"]),
        ];
        let refusal = goose_corridor(&chain, &crate::agent_kind::goose::GooseLaunchProgram::Goose)
            .expect_err("a chained trampoline must be refused");
        assert!(refusal.contains("unclassified process"), "{refusal}");
    }
}
