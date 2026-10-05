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

// Each integrated kind's corridor over a walked chain (which processes may
// sit between a hook and the agent's foreground process, and which one is
// the emitter) lives in its own file; the walk, the shared trampoline and
// runtime rules, and the platform reads stay here.
mod claude;
mod codex;
mod grok;
mod omp;
pub(crate) use claude::foreground_claude_emitter;
pub(crate) use codex::foreground_codex_emitter;
pub(crate) use grok::foreground_grok_emitter;
pub(crate) use omp::foreground_omp_emitter;

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
/// examined". A walk that ran but did not see this supervisor's own
/// process (an empty `/proc`, a zero-size `sysctl` answer) is `Err` for the
/// same reason: the caller is always in its own table, so a table without
/// it was not really read. A problem confined to ONE process (an unreadable
/// row on Linux) lands in the returned `Vec<String>` instead, so a single
/// odd process does not blind the scan to every other one.
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
    let (table, soft_errors) = imp::snapshot()?;
    require_own_row(table, soft_errors, std::process::id())
}

/// The self-witness check behind [`snapshot`]: a table that lacks `own_pid`
/// is an error, anything else passes through unchanged.
///
/// Only presence is checked, deliberately. This catches a walk that came
/// back empty or truncated without failing; it is not a general test of
/// whether every process was visible, and a process that hides from the
/// walk while this supervisor's own row is present is out of its scope.
/// Pure over its inputs so the refusal is testable without a broken
/// process table.
///
/// On Linux the walk keeps only `/proc` entries owned by this euid, and the
/// kernel shows a non-dumpable process's entry as root's. A supervisor made
/// non-dumpable (`PR_SET_DUMPABLE` 0, as some hardening does) would
/// therefore fail every walk here, and Stop and Delete would be unconfirmed
/// on every host without a systemd user manager.
///
/// The first few of the walk's per-process errors, and how many there were,
/// are carried into the refusal, since one of them is usually why this
/// supervisor's row was missing.
fn require_own_row(
    table: ProcessTable,
    soft_errors: Vec<String>,
    own_pid: u32,
) -> Result<(ProcessTable, Vec<String>), String> {
    if !table.contains_key(&own_pid) {
        let mut message = format!(
            "the process table walk did not include this supervisor (pid {own_pid}), so it cannot \
             be trusted to show the processes it is meant to find"
        );
        // A few examples and a count, not the whole list: a policy that denies
        // every read yields one error per process this user owns, and the
        // sweep's own report is bounded.
        if !soft_errors.is_empty() {
            message.push_str("; errors during the walk: ");
            message.push_str(&soft_errors[..soft_errors.len().min(3)].join("; "));
            if soft_errors.len() > 3 {
                message.push_str(&format!("; and {} more", soft_errors.len() - 3));
            }
        }
        return Err(message);
    }
    Ok((table, soft_errors))
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
/// (A recorded chain read back from a file that carries such a link anyway
/// is refused at anchoring; see [`anchor_chain`].)
pub(crate) const MAX_ARGV_BYTES_PER_PROCESS: usize = 64 * 1024;

/// Whole-walk command-line budget. Unlike the per-process bound this one
/// refuses the walk: megabytes of argv across the ancestry is not a
/// foreground runtime, it is a denial-of-observation, and accepting the
/// report without the evidence would bless exactly the gap the caps
/// exist to close.
pub(crate) const MAX_ARGV_BYTES_PER_WALK: usize = 1024 * 1024;

/// One edge of an attribution chain: the process, its parent, the kernel
/// start token distinguishing it from PID reuse, and the image, argv and
/// working-directory evidence captured while the edge was observed.
///
/// `exe` is REQUIRED — an unreadable image ends collection at that link,
/// so a chain never carries an edge without one — while `argv` is `None`
/// whenever that process's command line was unreadable or exceeded its
/// per-process bound (see the constants above for why those differ).
/// `cwd` exists for OMP alone: a Bun runtime may name its entry point
/// relative to its own working directory, and resolving that spelling must
/// not need the process to still be alive. It is `None` wherever the
/// platform offers no read (macOS) or the read failed.
#[derive(Debug, Clone)]
pub(crate) struct ChainLink {
    pub(crate) pid: u32,
    pub(crate) ppid: u32,
    pub(crate) start: u64,
    pub(crate) exe: Vec<u8>,
    pub(crate) argv: Option<Vec<Vec<u8>>>,
    pub(crate) cwd: Option<Vec<u8>>,
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

/// Refuse a chain whose command-line evidence totals past
/// [`MAX_ARGV_BYTES_PER_WALK`]. Re-checked over the anchored slice rather
/// than trusted from collection's own running count, because an anchored
/// chain may come from a report file rather than from [`collect_ancestry`],
/// and the bound must hold over the evidence the corridors actually read.
fn check_walk_argv_budget(chain: &[ChainLink]) -> Result<(), String> {
    if chain.iter().map(link_argv_bytes).sum::<usize>() > MAX_ARGV_BYTES_PER_WALK {
        return Err(WALK_BUDGET_EXCEEDED.to_string());
    }
    Ok(())
}

/// The refusal for a chain that never reaches the session's pane.
const NOT_ATTRIBUTABLE: &str = "the hook cannot be attributed to this session's foreground pane";

/// The refusal both walk-budget checks give.
const WALK_BUDGET_EXCEEDED: &str =
    "the hook ancestry's command lines exceed the attribution budget";

/// Command-line bytes one link carries, the unit both argv budgets are
/// stated in.
fn link_argv_bytes(link: &ChainLink) -> usize {
    link.argv.iter().flatten().map(Vec::len).sum()
}

/// Where an attribution chain must end: the session's owned pane process.
///
/// `start` is the pane process's kernel start token while it is alive, and
/// `None` once it has exited but tmux still lists the pane (`remain-on-exit`):
/// a dead process has no start token left to read, so only its pid can be
/// compared. Every launch runs a new pane process, which is what ties a chain
/// to the launch it was collected under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PaneAnchor {
    pub(crate) pid: u32,
    pub(crate) start: Option<u64>,
}

/// A reporting process's collected ancestry, and why collection stopped
/// where it did.
///
/// `ended` is the reason collection stopped before running out of parents
/// (an exited or unreadable process, an image that could not be read, a
/// process that changed while it was observed, the walk budget), or `None`
/// when it simply reached the top. It matters only when the pane turns out
/// not to be in `links`: then it, rather than a generic "not attributable",
/// is the honest refusal, since it says why the chain never got that far.
#[derive(Debug, Clone)]
pub(crate) struct Ancestry {
    pub(crate) links: Vec<ChainLink>,
    pub(crate) ended: Option<String>,
}

/// Collect the ancestry of a reporting process, starting with the process
/// itself, capturing image and argv evidence per edge (and the working
/// directory of the interpreters whose entry point OMP attribution resolves).
///
/// This is the first half of attribution, and it decides NOTHING about
/// sessions or vendors: it does not know which pane it should reach, so it
/// climbs until the ancestry ends (a parent of 0, a loop), until a process is
/// gone or its image unreadable, until [`MAX_ATTRIBUTION_ANCESTORS`] edges,
/// or until the next edge's argv would push the chain past
/// [`MAX_ARGV_BYTES_PER_WALK`]. Each of those ends the chain rather than
/// failing it, because the pane may well lie below that point; [`anchor_chain`]
/// is what refuses a chain that never reached the pane, citing
/// [`Ancestry::ended`]. The one hard failure is the reporter itself: it must
/// be live, readable, and still carry the start token the caller observed.
///
/// Collection reads every ancestor up to where it stops, including ones far
/// above any pane (the tmux server, the service manager). Those reads cost a
/// handful of `/proc` reads per hook; argv is read from each, and a process
/// whose memory map is wedged could slow that read, which is accepted as
/// the price of not needing to know the pane.
///
/// Before returning, every collected edge and image is re-read, and the chain
/// is cut just below the first link that no longer matches. A process that
/// exec'd or exited between its observation and the re-read is no longer the
/// process the chain describes, so neither it nor anything above it is handed
/// on as evidence. Cutting rather than failing keeps a change far above the
/// pane from refusing a report whose own corridor is intact, while a change at
/// or below the pane drops the pane from the chain, which [`anchor_chain`]
/// refuses. A changed reporter fails outright. Argv and working directory are
/// per-observation evidence, not identity, so they are not compared.
pub(crate) fn collect_ancestry(reporter: ProcessIdentity) -> Result<Ancestry, String> {
    let mut chain: Vec<ChainLink> = Vec::with_capacity(8);
    let mut ended = None;
    let mut pid = reporter.pid;
    let mut argv_total = 0usize;
    for _ in 0..MAX_ATTRIBUTION_ANCESTORS {
        let (parent, start) = match read_process(pid) {
            Ok(Some((parent, start, ProcessState::Running))) => (parent, start),
            Err(error) if !chain.is_empty() => {
                ended = Some(error);
                break;
            }
            Err(error) => return Err(error),
            Ok(_) if chain.is_empty() => {
                return Err("the hook ancestry is no longer live".to_string());
            }
            Ok(_) => {
                ended = Some("the hook ancestry is no longer live".to_string());
                break;
            }
        };
        if chain.is_empty() && start != reporter.start {
            return Err("the hook connection's process identity changed".to_string());
        }
        let exe = match imp::process_exe(pid) {
            Ok(exe) => exe,
            Err(error) if chain.is_empty() => return Err(error),
            Err(error) => {
                ended = Some(error);
                break;
            }
        };
        let argv = read_process_argv(pid);
        let link_argv: usize = argv.iter().flatten().map(Vec::len).sum();
        if argv_total + link_argv > MAX_ARGV_BYTES_PER_WALK {
            ended = Some(WALK_BUDGET_EXCEEDED.to_string());
            break;
        }
        argv_total += link_argv;
        // Only an interpreter's entry point is ever resolved against its
        // working directory (OMP under Bun, and Node to name the unsupported
        // shape), so only those links record one: a report file then carries
        // no other process's directory.
        let cwd = (omp::is_bun_image(&exe) || omp::is_node_image(&exe))
            .then(|| imp::process_cwd(pid))
            .flatten();
        chain.push(ChainLink {
            pid,
            ppid: parent,
            start,
            exe,
            argv,
            cwd,
        });
        if parent == 0 || chain.iter().any(|link| link.pid == parent) {
            break;
        }
        pid = parent;
    }
    let unchanged = unchanged_prefix(&chain, |link| {
        read_process(link.pid).ok().flatten()
            == Some((link.ppid, link.start, ProcessState::Running))
            && imp::process_exe(link.pid).ok().as_ref() == Some(&link.exe)
    });
    if unchanged == 0 {
        return Err("the hook ancestry changed during attribution".to_string());
    }
    if unchanged < chain.len() {
        chain.truncate(unchanged);
        ended = Some("the hook ancestry changed during attribution".to_string());
    }
    Ok(Ancestry {
        links: chain,
        ended,
    })
}

/// How many links, from the reporter up, still pass `still_same`: the
/// length collection may keep. Separate from the live re-read so the cut is
/// testable without processes that change on cue.
fn unchanged_prefix(chain: &[ChainLink], still_same: impl Fn(&ChainLink) -> bool) -> usize {
    chain.iter().take_while(|link| still_same(link)).count()
}

/// Cut a collected chain at the session's pane process: the second half of
/// attribution, and the part that ties the chain to one launch.
///
/// The returned slice runs from the reporter up to and including the pane
/// link, the shape every per-kind corridor reads (reporter first, pane anchor
/// last). The pane must appear in the chain with the anchor's pid and, when
/// the anchor carries one, its start token; a chain that never reached it —
/// too deep, broken by an unreadable process, or collected under another
/// launch's pane — refuses, naming `ended` as context when collection gave
/// one.
///
/// The chain may come from a report file, so the kept slice is checked for
/// the shape collection guarantees and the corridors rely on: each link the
/// child of the one above it (Claude's corridor is positional), no pid twice,
/// at most [`MAX_ATTRIBUTION_ANCESTORS`] links, no link over the per-process
/// argv budget, and the walk budget over the whole slice. Evidence above the
/// pane never counts against a report. A recorded link over the per-process
/// budget refuses rather than counting as missing evidence: collection never
/// produces one, so it can only come from a file this build did not write.
pub(crate) fn anchor_chain<'c>(
    chain: &'c [ChainLink],
    ended: Option<&str>,
    pane: PaneAnchor,
) -> Result<&'c [ChainLink], String> {
    let index = chain
        .iter()
        .take(MAX_ATTRIBUTION_ANCESTORS)
        .position(|link| link.pid == pane.pid && pane.start.is_none_or(|start| link.start == start))
        .ok_or_else(|| match ended {
            // Collection rarely reaches the top on its own: it usually stops
            // at an ancestor it may not read (pid 1, for one), so its reason
            // is context for the refusal, never the refusal itself.
            Some(ended) => format!("{NOT_ATTRIBUTABLE} (ancestry collection stopped: {ended})"),
            None => NOT_ATTRIBUTABLE.to_string(),
        })?;
    let kept = &chain[..=index];
    let contiguous = kept.windows(2).all(|pair| pair[0].ppid == pair[1].pid);
    let distinct = kept
        .iter()
        .enumerate()
        .all(|(i, link)| kept[..i].iter().all(|earlier| earlier.pid != link.pid));
    if !contiguous || !distinct {
        return Err("the hook ancestry is not one unbroken chain of parents".to_string());
    }
    if kept
        .iter()
        .any(|link| link_argv_bytes(link) > MAX_ARGV_BYTES_PER_PROCESS)
    {
        return Err(
            "a command line in the hook ancestry exceeds the per-process evidence budget"
                .to_string(),
        );
    }
    check_walk_argv_budget(kept)?;
    Ok(kept)
}

/// Walk the live ancestry from a hook connection's process to the owned
/// pane: [`collect_ancestry`] followed by [`anchor_chain`] at the pane's pid.
/// The pane is matched by pid alone, as the live walk always matched it;
/// collection has already re-verified every link it kept. It decides
/// NOTHING about vendors — the per-kind step applies its own corridor and
/// runtime recognition to the returned chain.
pub(crate) fn walk_to_pane(peer: ProcessIdentity, pane_pid: u32) -> Result<Vec<ChainLink>, String> {
    let ancestry = collect_ancestry(peer)?;
    anchor_chain(
        &ancestry.links,
        ancestry.ended.as_deref(),
        PaneAnchor {
            pid: pane_pid,
            start: None,
        },
    )
    .map(<[ChainLink]>::to_vec)
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
/// session-hosting image other than the emitter this corridor counts.
/// Such an intermediary means the report traveled through a
/// different harness, and refuses with its own message so the diagnostic
/// names the mechanism rather than a generic unclassified intermediary.
///
/// The runtime names are the executable basenames that identify each
/// integrated kind (`agent_kind::executable_basename`), so a new kind is
/// recognized here as soon as it names its executable there.
fn is_other_session_runtime(exe: &[u8]) -> bool {
    let name = exe.rsplit(|byte| *byte == b'/').next().unwrap_or_default();
    farhelm_proto::AgentKind::ALL
        .iter()
        .filter_map(|kind| crate::agent_kind::executable_basename(*kind))
        .any(|runtime| runtime.as_bytes() == name)
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
    /// Failures (gone, foreign uid, non-dumpable) end collection at that
    /// process, which refuses the report when the pane lies above it.
    pub(super) fn process_exe(pid: u32) -> Result<Vec<u8>, String> {
        use std::os::unix::ffi::OsStrExt;
        std::fs::read_link(format!("/proc/{pid}/exe"))
            .map(|path| path.as_os_str().as_bytes().to_vec())
            .map_err(|error| format!("reading process {pid} executable: {error}"))
    }

    /// The process's current working directory, for resolving an entry
    /// point the process was handed as a relative spelling. `None` on any
    /// failure: the caller treats it as missing evidence.
    pub(super) fn process_cwd(pid: u32) -> Option<Vec<u8>> {
        use std::os::unix::ffi::OsStrExt;
        std::fs::read_link(format!("/proc/{pid}/cwd"))
            .ok()
            .map(|path| path.as_os_str().as_bytes().to_vec())
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

    /// No working-directory read on macOS: OMP's relative entry resolution
    /// has never had one here (it read `/proc`), so a relative spelling
    /// stays unresolved exactly as before.
    pub(super) fn process_cwd(_pid: u32) -> Option<Vec<u8>> {
        None
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
    use super::{claude::*, codex::*, grok::*, omp::*};

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

    /// Spec: a process-table walk that does not contain the caller's own
    /// pid is an error, while one that does passes through with its soft
    /// errors intact.
    ///
    /// Why: Stop and Delete find a session's processes in the table. A walk
    /// that silently came back empty (an unmounted `/proc`, a zero-size
    /// `sysctl` answer) found nothing to stop, so cleanup reported success
    /// having examined nothing; the caller is always in its own table, so
    /// its absence is the one cheap proof the walk did not happen. The
    /// refusal keeps the walk's per-process errors, which usually say why.
    #[farhelm_testtrace::test]
    fn a_snapshot_without_its_own_row_is_an_error() {
        assert!(
            require_own_row(ProcessTable::new(), Vec::new(), 4242).is_err(),
            "an empty table must not read as \"nothing running\""
        );
        let refusal = require_own_row(
            ProcessTable::new(),
            vec!["pid 4242: permission denied".to_string()],
            4242,
        )
        .expect_err("an empty table refuses whatever its soft errors");
        assert!(
            refusal.contains("pid 4242: permission denied"),
            "the refusal must say why the row was missing: {refusal}"
        );
        let many = (0..1000).map(|pid| format!("pid {pid}: denied")).collect();
        let refusal = require_own_row(ProcessTable::new(), many, 4242)
            .expect_err("an empty table refuses whatever its soft errors");
        assert!(
            refusal.contains("pid 2: denied; and 997 more") && !refusal.contains("pid 3: denied"),
            "a flood of per-process errors is summarized, not copied whole: {refusal}"
        );
        let mut others = ProcessTable::new();
        others.insert(17, (1, 1_000));
        assert!(
            require_own_row(others, Vec::new(), 4242).is_err(),
            "a table missing only the caller must still refuse"
        );
        let mut table = ProcessTable::new();
        table.insert(4242, (1, 1_000));
        table.insert(17, (4242, 2_000));
        let (passed, soft) =
            require_own_row(table.clone(), vec!["pid 99: unreadable".to_string()], 4242)
                .expect("a table containing the caller passes");
        assert_eq!(passed, table);
        assert_eq!(soft, vec!["pid 99: unreadable".to_string()]);
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

    /// Spec: collection starts at the reporter, which must still be the
    /// process the caller observed, and climbs past it with evidence.
    ///
    /// Why: the hook collects its own ancestry with no pane to stop at, so
    /// collection must carry the reporter's identity check on its own; a
    /// recycled pid answering for the reporter would otherwise lend its
    /// ancestry to a report it never made.
    #[farhelm_testtrace::test]
    fn collection_starts_at_the_live_reporter_and_climbs() {
        let me = std::process::id();
        let peer = ProcessIdentity::read(me).expect("this process is live");
        let parent = read_process(me)
            .expect("read this process")
            .expect("this process is listed")
            .0;
        assert!(
            ProcessIdentity::read(parent).is_some() && imp::process_exe(parent).is_ok(),
            "fixture premise: the test runner's parent is live and its image readable"
        );
        let chain = collect_ancestry(peer)
            .expect("a live process collects its ancestry")
            .links;
        assert_eq!(chain[0].pid, me);
        assert_eq!(chain[0].start, peer.start);
        assert_eq!(
            chain[1].pid, parent,
            "collection climbs to the reporter's parent"
        );
        let stale = ProcessIdentity {
            pid: me,
            start: peer.start.wrapping_add(1),
        };
        let error = collect_ancestry(stale).expect_err("a recycled reporter refuses");
        assert!(error.contains("identity changed"), "{error}");
    }

    /// Spec: the live walk keeps the chain from the reporter up to the named
    /// pane process, and refuses a pid that is not among its ancestors.
    ///
    /// Why: this is the composition the supervisor's live admission runs;
    /// the split must reach the same pane the old single walk reached.
    #[farhelm_testtrace::test]
    fn the_live_walk_stops_at_the_pane_and_refuses_strangers() {
        let me = std::process::id();
        let peer = ProcessIdentity::read(me).expect("this process is live");
        let parent = read_process(me).expect("read").expect("listed").0;
        let chain = walk_to_pane(peer, parent).expect("the parent is in the ancestry");
        assert_eq!(
            chain.iter().map(|link| link.pid).collect::<Vec<_>>(),
            [me, parent]
        );
        let mut stranger = crate::procs::sleeper::spawn(&[]);
        let refused = walk_to_pane(peer, stranger.id());
        let _ = stranger.kill();
        let _ = stranger.wait();
        let error = refused.expect_err("a process outside the ancestry is not its pane");
        assert!(error.starts_with(NOT_ATTRIBUTABLE), "{error}");
    }

    /// A synthetic five-link ancestry, each link the child of the next:
    /// reporter 12, trampoline 11, runtime 10 as the pane process, then the
    /// tmux server 9 and its parent 8 above the pane, which collection
    /// gathers because it cannot know where the pane is.
    fn ancestry_above_a_pane() -> Vec<ChainLink> {
        let mut chain = vec![
            corridor_link(12, "/opt/test/bin/farhelm", &hook_argv()),
            corridor_link(11, "/bin/sh", &["/bin/sh", "-c", "farhelm internal hook"]),
            corridor_link(10, "/opt/agent/bin/agent", &["agent"]),
            corridor_link(9, "/usr/bin/tmux", &["tmux"]),
            corridor_link(8, "/usr/lib/systemd/systemd", &["systemd", "--user"]),
        ];
        for link in &mut chain {
            link.ppid = link.pid - 1;
        }
        chain
    }

    /// The anchor of [`ancestry_above_a_pane`]'s pane process while alive.
    const LIVE_PANE: PaneAnchor = PaneAnchor {
        pid: 10,
        start: Some(10_000),
    };

    /// Spec: anchoring keeps the chain from the reporter through the pane
    /// process, matched by pid and start token while the pane process is
    /// alive and by pid alone once it has exited.
    ///
    /// Why: the anchor is what ties recorded evidence to one launch. Every
    /// launch runs a new pane process, so a chain collected under an
    /// earlier launch names another pid (or the same pid with another
    /// start token) and must not attribute to this one.
    #[farhelm_testtrace::test]
    fn anchoring_keeps_the_chain_up_to_the_launch_pane() {
        let chain = ancestry_above_a_pane();
        let kept = anchor_chain(&chain, None, LIVE_PANE).expect("the live pane anchors");
        assert_eq!(
            kept.iter().map(|link| link.pid).collect::<Vec<_>>(),
            [12, 11, 10]
        );
        let dead = PaneAnchor {
            pid: 10,
            start: None,
        };
        let kept = anchor_chain(&chain, None, dead).expect("a dead pane anchors by pid");
        assert_eq!(kept.len(), 3);
        let recycled = PaneAnchor {
            pid: 10,
            start: Some(10_001),
        };
        let error = anchor_chain(&chain, None, recycled).expect_err("another launch's pane");
        assert!(error.contains("cannot be attributed"), "{error}");
        let elsewhere = PaneAnchor {
            pid: 7,
            start: None,
        };
        let error = anchor_chain(
            &chain[..2],
            Some("the hook ancestry is no longer live"),
            elsewhere,
        )
        .expect_err("a chain that never reached the pane refuses");
        assert!(error.starts_with(NOT_ATTRIBUTABLE), "{error}");
        assert!(
            error.contains("no longer live"),
            "why collection stopped is kept as context: {error}"
        );
        let deep: Vec<ChainLink> = (0..=MAX_ATTRIBUTION_ANCESTORS as u32)
            .map(|i| {
                let mut link = corridor_link(1000 - i, "/bin/sh", &["sh"]);
                link.ppid = 999 - i;
                link
            })
            .collect();
        let past_depth = PaneAnchor {
            pid: deep[MAX_ATTRIBUTION_ANCESTORS].pid,
            start: None,
        };
        let error = anchor_chain(&deep, None, past_depth).expect_err("a pane past the depth bound");
        assert!(error.starts_with(NOT_ATTRIBUTABLE), "{error}");
    }

    /// Spec: an anchored chain must be one unbroken line of parents with no
    /// pid twice.
    ///
    /// Why: Claude's corridor is positional ("the pane's direct child"), and
    /// a chain read from a report file is input, not collection's own
    /// output; a chain with a gap or a repeat would let position mean
    /// something it does not.
    #[farhelm_testtrace::test]
    fn anchoring_refuses_a_broken_or_looping_chain() {
        let mut gap = ancestry_above_a_pane();
        gap[0].ppid = 99;
        let error = anchor_chain(&gap, None, LIVE_PANE).expect_err("a gap refuses");
        assert!(error.contains("unbroken chain"), "{error}");
        let mut repeat = ancestry_above_a_pane();
        repeat[1] = repeat[0].clone();
        repeat[1].ppid = 10;
        repeat[0].ppid = 12;
        let error = anchor_chain(&repeat, None, LIVE_PANE).expect_err("a repeat refuses");
        assert!(error.contains("unbroken chain"), "{error}");
    }

    /// Spec: the walk's command-line budget is judged over the anchored
    /// slice only: evidence above the pane never refuses a report, while
    /// evidence below it does, and so does one link over the per-process
    /// budget.
    ///
    /// Why: collection gathers ancestors above the pane (the tmux server,
    /// the service manager) whose command lines say nothing about the
    /// report. Counting them would let an unrelated long command line
    /// refuse a valid report, and not counting the reporter's side would
    /// let a denial-of-observation through.
    #[farhelm_testtrace::test]
    fn the_argv_budget_covers_only_the_anchored_slice() {
        let oversized = "x".repeat(MAX_ARGV_BYTES_PER_PROCESS);
        let many: Vec<&str> = std::iter::repeat_n(oversized.as_str(), 17).collect();
        let mut above = ancestry_above_a_pane();
        above[4].argv = Some(many.iter().map(|arg| arg.as_bytes().to_vec()).collect());
        assert!(anchor_chain(&above, None, LIVE_PANE).is_ok());
        let mut one_long = ancestry_above_a_pane();
        one_long[1].argv = Some(vec![oversized.as_bytes().to_vec(); 2]);
        let error =
            anchor_chain(&one_long, None, LIVE_PANE).expect_err("over the per-process budget");
        assert!(error.contains("per-process evidence budget"), "{error}");
        // Seventeen trampolines of exactly one per-process budget each sit
        // below the pane: no single link is over, the slice as a whole is.
        let mut many_links: Vec<ChainLink> = (0..17u32)
            .map(|i| {
                let mut link = corridor_link(100 - i, "/bin/sh", &[oversized.as_str()]);
                link.ppid = 99 - i;
                link
            })
            .collect();
        many_links.push(corridor_link(83, "/opt/agent/bin/agent", &["agent"]));
        let pane = PaneAnchor {
            pid: 83,
            start: None,
        };
        let error = anchor_chain(&many_links, None, pane).expect_err("over the walk budget");
        assert!(error.contains("attribution budget"), "{error}");
    }

    /// Spec: collection keeps the links from the reporter up to the first
    /// one that changed while it was observed, and nothing above it.
    ///
    /// Why: a process that exec'd or exited mid-collection no longer is the
    /// process the chain describes. Cutting there keeps a change far above
    /// the pane harmless while a change at or below the pane drops the pane,
    /// which anchoring then refuses.
    #[farhelm_testtrace::test]
    fn a_link_that_changed_cuts_the_chain_below_it() {
        let chain = ancestry_above_a_pane();
        let above_pane = unchanged_prefix(&chain, |link| link.pid != 9);
        assert_eq!(above_pane, 3);
        assert!(anchor_chain(&chain[..above_pane], None, LIVE_PANE).is_ok());
        let below_pane = unchanged_prefix(&chain, |link| link.pid != 11);
        assert_eq!(below_pane, 1);
        assert!(anchor_chain(&chain[..below_pane], None, LIVE_PANE).is_err());
        assert_eq!(unchanged_prefix(&chain, |link| link.pid != 12), 0);
    }

    /// Spec: a relative OMP entry spelling resolves against the working
    /// directory the chain recorded for that process.
    ///
    /// Why: a recorded chain is read after the process it describes may
    /// have exited, so resolution cannot fall back to the live process's
    /// working directory; without the recorded one, a launch that named its
    /// entry relatively would stop attributing.
    #[farhelm_testtrace::test]
    fn a_relative_omp_entry_resolves_against_the_recorded_working_directory() {
        let scratch = farhelm_teststate::tempdir().expect("fixture directory");
        let relative = "node_modules/@oh-my-pi/pi-coding-agent/dist/cli.js";
        let entry = scratch.path().join(relative);
        std::fs::create_dir_all(entry.parent().expect("bundle parent")).expect("bundle layout");
        std::fs::write(&entry, b"fixture entry").expect("entry file");
        let cwd = os_bytes(scratch.path());
        let resolved = resolved_omp_entry(Some(&cwd), relative.as_bytes())
            .expect("the relative spelling resolves");
        assert!(is_omp_bundle_entry(&resolved), "{resolved:?}");
        assert!(
            resolved_omp_entry(None, b"./cli.js").is_none(),
            "a relative spelling without a recorded directory resolves to nothing"
        );
    }

    /// Spec: OMP's corridor resolves a Bun runtime's relative entry spelling
    /// against the working directory recorded on that link.
    ///
    /// Why: the corridor reads a recorded chain, not live processes; this
    /// pins that it uses the emitter link's own recorded directory.
    #[farhelm_testtrace::test]
    fn the_omp_corridor_reads_the_recorded_working_directory() {
        let scratch = farhelm_teststate::tempdir().expect("fixture directory");
        let relative = "node_modules/@oh-my-pi/pi-coding-agent/dist/cli.js";
        let entry = scratch.path().join(relative);
        std::fs::create_dir_all(entry.parent().expect("bundle parent")).expect("bundle layout");
        std::fs::write(&entry, b"fixture entry").expect("entry file");
        let mut runtime = corridor_link(11, "/opt/bun/bin/bun", &["bun", relative]);
        runtime.cwd = Some(os_bytes(scratch.path()));
        let chain = vec![
            corridor_link(12, "/opt/test/bin/farhelm", &omp_hook_argv()),
            runtime,
            corridor_link(10, "/bin/bash", &["-bash"]),
        ];
        let emitter = omp_corridor(&chain, &crate::agent_kind::omp::OmpLaunchProgram::Omp)
            .expect("the relative entry admits through the recorded directory");
        assert_eq!(emitter.pid, 11);
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
            cwd: None,
        }
    }

    fn corridor_link_no_argv(pid: u32, exe: &str) -> ChainLink {
        ChainLink {
            pid,
            ppid: 0,
            start: u64::from(pid) * 1_000,
            exe: exe.as_bytes().to_vec(),
            argv: None,
            cwd: None,
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

    /// The pane anchor is accepted by position: a wrapper that IS the
    /// pane root (here the login shell the pane reports) is
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

    /// The anchor exemption must not admit arbitrary surviving launch
    /// ancestry. A package wrapper below a separate pane shell is an
    /// intermediary even when the only native Codex is its direct child.
    #[farhelm_testtrace::test]
    fn a_package_wrapper_below_a_separate_pane_anchor_is_refused() {
        let chain = vec![
            corridor_link(13, "/opt/test/bin/farhelm", &hook_argv()),
            corridor_link(12, "/opt/test/bin/codex", &["codex"]),
            corridor_link(11, "/usr/bin/node", &["node", "codex.js"]),
            corridor_link(10, "/bin/bash", &["bash", "launch.sh"]),
        ];
        let refusal = codex_corridor(&chain).expect_err("only the pane anchor is exempt");
        assert!(refusal.contains("unclassified intermediary"), "{refusal}");
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

    /// Grok's supported foreground is one native image with a private
    /// backend flag in its option region. A flag after `--` is user payload,
    /// so treating it as ownership evidence would admit the shared leader.
    #[farhelm_testtrace::test]
    fn grok_requires_no_leader_before_the_option_boundary() {
        let admitted = vec![
            corridor_link(
                12,
                "/opt/test/bin/farhelm",
                &["farhelm", "internal", "hook", "--vendor", "grok"],
            ),
            corridor_link(
                11,
                "/bin/sh",
                &[
                    "sh",
                    "-c",
                    "/opt/test/bin/farhelm internal hook --vendor grok",
                ],
            ),
            corridor_link(
                10,
                "/home/user/.grok/downloads/grok-linux-x86_64",
                &["grok", "--no-leader"],
            ),
        ];
        let emitter = grok_corridor(&admitted).expect("the supported Grok corridor is admitted");
        assert_eq!(emitter.pid, 10);

        for argv in [
            vec!["grok"],
            vec!["grok", "--", "--no-leader"],
            vec!["grok", "--no-leader=false"],
        ] {
            let chain = vec![
                corridor_link(
                    11,
                    "/opt/test/bin/farhelm",
                    &["farhelm", "internal", "hook", "--vendor", "grok"],
                ),
                corridor_link(10, "/opt/test/bin/grok", &argv),
            ];
            let refusal =
                grok_corridor(&chain).expect_err("missing exact ownership evidence must refuse");
            assert!(refusal.contains("--no-leader"), "{refusal}");
        }
    }

    /// Another Grok image or another harness in the ancestry means the hook
    /// cannot be attributed to the one top-level foreground conversation.
    #[farhelm_testtrace::test]
    fn grok_rejects_nested_and_foreign_runtime_corridors() {
        let nested = vec![
            corridor_link(
                13,
                "/opt/test/bin/farhelm",
                &["farhelm", "internal", "hook", "--vendor", "grok"],
            ),
            corridor_link(
                12,
                "/home/user/.grok/downloads/grok-linux-x86_64",
                &["grok", "--no-leader"],
            ),
            corridor_link(
                11,
                "/home/user/.grok/downloads/grok-linux-x86_64",
                &["grok", "--no-leader"],
            ),
        ];
        let refusal = grok_corridor(&nested).expect_err("nested Grok must refuse");
        assert!(refusal.contains("nested Grok"), "{refusal}");

        let foreign = vec![
            corridor_link(
                13,
                "/opt/test/bin/farhelm",
                &["farhelm", "internal", "hook", "--vendor", "grok"],
            ),
            corridor_link(12, "/opt/test/bin/codex", &["codex"]),
            corridor_link(
                11,
                "/home/user/.grok/downloads/grok-linux-x86_64",
                &["grok", "--no-leader"],
            ),
            corridor_link(10, "/bin/bash", &["-bash"]),
        ];
        let refusal = grok_corridor(&foreign).expect_err("foreign runtime must refuse");
        assert!(
            refusal.contains("another session-hosting runtime"),
            "{refusal}"
        );
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

    /// Spec: for a launch of the installed `omp` command, a Bun or Node pane
    /// process that is not the emitter refuses, including one whose
    /// arguments could not be read; a pane that is itself the reporting
    /// runtime and a package launcher pane of a Bun launch stay admitted.
    /// (A shell pane above the runtime is admitted by
    /// `an_installed_omp_runtime_with_a_direct_hook_child_is_admitted`.)
    ///
    /// Why: the pane is accepted by position, and a runtime whose arguments
    /// are missing (an over-budget command line reads as none) is not
    /// recognized as a runtime. Without this check, a nested OMP below such
    /// a pane was the only runtime found, so its report was taken for the
    /// session's and Resume reopened the nested conversation. The admitted
    /// controls show the refusal is confined to the shape that cannot be a
    /// legitimate `omp` launch.
    #[farhelm_testtrace::test]
    fn an_omp_launch_refuses_an_unclassified_runtime_pane_above_the_emitter() {
        for pane in [
            corridor_link_no_argv(11, "/opt/bun/bin/bun"),
            corridor_link_no_argv(11, "/opt/node/bin/node"),
            corridor_link(11, "/opt/bun/bin/bun", &["bun", "/opt/other/tool.js"]),
        ] {
            let chain = vec![
                corridor_link(13, "/opt/test/bin/farhelm", &omp_hook_argv()),
                omp_runtime_link(12, &[]),
                pane.clone(),
            ];
            let refusal = omp_corridor(&chain, &crate::agent_kind::omp::OmpLaunchProgram::Omp)
                .expect_err("a runtime pane that is not the emitter must refuse");
            assert!(
                refusal.contains("not the reporting OMP runtime"),
                "{pane:?}: {refusal}"
            );
        }

        let runtime_pane = vec![
            corridor_link(13, "/opt/test/bin/farhelm", &omp_hook_argv()),
            omp_runtime_link(12, &[]),
        ];
        let emitter = omp_corridor(
            &runtime_pane,
            &crate::agent_kind::omp::OmpLaunchProgram::Omp,
        )
        .expect("a Bun pane that is the reporting runtime stays admitted");
        assert_eq!(emitter.pid, 12);

        let launcher_pane = vec![
            corridor_link(13, "/opt/test/bin/farhelm", &omp_hook_argv()),
            omp_runtime_link(12, &[]),
            corridor_link(
                11,
                "/opt/bun/bin/bun",
                &["bun", "x", "@oh-my-pi/pi-coding-agent"],
            ),
        ];
        let emitter = omp_corridor(
            &launcher_pane,
            &crate::agent_kind::omp::OmpLaunchProgram::Bun,
        )
        .expect("a Bun launch's package launcher pane stays admitted");
        assert_eq!(emitter.pid, 12);
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
    /// this launch's shape. (The pane anchor is otherwise accepted by
    /// position, apart from an installed-`omp` launch's Bun or Node pane
    /// that is not the emitter; this test keeps a shell anchor above the
    /// launcher to pin the middle-link rule rather than the anchor rule.)
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
        let resolved = resolved_omp_entry(None, &os_bytes(&shim)).expect("the shim resolves");
        assert!(
            is_omp_bundle_entry(&resolved),
            "the canonical path names the bundle: {resolved:?}"
        );
        assert!(
            resolved_omp_entry(None, b"/nonexistent-omp-entry-xyz").is_none(),
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

    // Claude's positional corridor. The images below are spelled the way
    // the native installer lays Claude out (a version-named executable) on
    // purpose: the corridor must decide by position alone, so a test that
    // happened to use a `claude`-named image could not tell a positional
    // rule from an image rule.

    /// The hook command the Claude injection installs, as the kernel
    /// captures it on the reporter.
    fn claude_hook_argv() -> Vec<&'static str> {
        vec![
            "/opt/test/bin/farhelm",
            "internal",
            "hook",
            "--vendor",
            "claude",
            "--announce",
        ]
    }

    /// The `sh -c` link Claude Code puts between itself and a hook command.
    fn claude_hook_trampoline(pid: u32) -> ChainLink {
        corridor_link(
            pid,
            "/usr/bin/dash",
            &[
                "/bin/sh",
                "-c",
                "'/opt/test/bin/farhelm' internal hook --vendor claude --announce",
            ],
        )
    }

    const CLAUDE_IMAGE: &str = "/home/user/.local/share/claude/versions/2.1.284";

    /// A plain launch as observed on Claude Code 2.1.284: the pane process
    /// IS Claude, which runs the hook through a non-exec'ing `sh -c`.
    ///
    /// Why this test matters: this is the shape every ordinary Claude
    /// session reports through, so refusing it would silently cost every
    /// session its hook-reported identity and leave Restart unavailable.
    #[farhelm_testtrace::test]
    fn claude_admits_a_hook_run_by_the_pane_process_through_a_trampoline() {
        let chain = vec![
            corridor_link(12, "/opt/test/bin/farhelm", &claude_hook_argv()),
            claude_hook_trampoline(11),
            corridor_link(10, CLAUDE_IMAGE, &["claude"]),
        ];
        let emitter = claude_corridor(&chain).expect("the foreground's own hook must be admitted");
        assert_eq!(emitter.pid, 10, "the emitter is the pane process");
        assert_eq!(emitter.start, 10_000);
    }

    /// The same launch when the hook's shell `exec`s the hook (bash does;
    /// dash on some versions does too): no trampoline link survives.
    ///
    /// Why this test matters: the emitter's position must not depend on
    /// which `/bin/sh` the host ships.
    #[farhelm_testtrace::test]
    fn claude_admits_a_hook_whose_shell_execd_it() {
        let chain = vec![
            corridor_link(12, "/opt/test/bin/farhelm", &claude_hook_argv()),
            corridor_link(10, CLAUDE_IMAGE, &["claude"]),
        ];
        let emitter = claude_corridor(&chain).expect("an exec'd hook must be admitted");
        assert_eq!(emitter.pid, 10);
    }

    /// A supported one-level wrapper launch: a resident wrapper is the
    /// pane process and Claude is its direct child. The wrapper's image
    /// and argv are irrelevant.
    ///
    /// Why this test matters: wrapper launches are a supported launch
    /// shape (`e2e/wrapper_launch.rs`), and they must keep hook capture.
    #[farhelm_testtrace::test]
    fn claude_admits_a_hook_under_a_one_level_wrapper() {
        let chain = vec![
            corridor_link(13, "/opt/test/bin/farhelm", &claude_hook_argv()),
            claude_hook_trampoline(12),
            corridor_link(11, CLAUDE_IMAGE, &["claude"]),
            corridor_link(10, "/opt/test/bin/wrapper", &["wrapper", "run", "/work"]),
        ];
        let emitter = claude_corridor(&chain).expect("a one-level wrapper must be admitted");
        assert_eq!(emitter.pid, 11, "the emitter is Claude, not the wrapper");
    }

    /// A shelled-out sub-agent: the foreground Claude (the pane) runs a
    /// shell for its Bash tool, the shell runs a second `claude`, and that
    /// child runs a reporting hook it picked up from settings.
    ///
    /// Why this test matters: this is the overwrite the corridor exists to
    /// refuse. The child inherits the session credential, so without this
    /// refusal its conversation would replace the one the user sees.
    #[farhelm_testtrace::test]
    fn claude_refuses_a_hook_run_by_a_shelled_out_child() {
        let chain = vec![
            corridor_link(14, "/opt/test/bin/farhelm", &claude_hook_argv()),
            claude_hook_trampoline(13),
            corridor_link(12, CLAUDE_IMAGE, &["claude", "-p", "do the thing"]),
            corridor_link(
                11,
                "/bin/bash",
                &[
                    "/bin/bash",
                    "-c",
                    "-l",
                    "eval 'claude -p \"do the thing\"' && pwd -P",
                ],
            ),
            corridor_link(10, CLAUDE_IMAGE, &["claude"]),
        ];
        let refusal = claude_corridor(&chain).expect_err("a nested child must be refused");
        assert!(refusal.contains("nested below"), "{refusal}");
    }

    /// The same child with its own hook shell exec'd away still refuses:
    /// skipping trampolines never moves the emitter upward past a real
    /// process.
    #[farhelm_testtrace::test]
    fn claude_refuses_a_shelled_out_child_whose_hook_shell_execd() {
        let chain = vec![
            corridor_link(13, "/opt/test/bin/farhelm", &claude_hook_argv()),
            corridor_link(12, CLAUDE_IMAGE, &["claude", "-p", "x"]),
            corridor_link(
                11,
                "/bin/bash",
                &["/bin/bash", "-c", "-l", "claude -p x; true"],
            ),
            corridor_link(10, CLAUDE_IMAGE, &["claude"]),
        ];
        claude_corridor(&chain).expect_err("a nested child must be refused");
    }

    /// A wrapper chain two levels deep refuses. This is the accepted cost
    /// of a rule that recognizes no executable: such a session cannot be
    /// restarted rather than widening the rule.
    #[farhelm_testtrace::test]
    fn claude_refuses_a_hook_under_a_two_level_wrapper() {
        let chain = vec![
            corridor_link(14, "/opt/test/bin/farhelm", &claude_hook_argv()),
            claude_hook_trampoline(13),
            corridor_link(12, CLAUDE_IMAGE, &["claude"]),
            corridor_link(11, "/bin/sh", &["sh", "/work/run-claude.sh"]),
            corridor_link(10, "/opt/test/bin/wrapper", &["wrapper", "run", "/work"]),
        ];
        claude_corridor(&chain).expect_err("a two-level wrapper must be refused");
    }

    /// A process that is not the hook invocation refuses however clean the
    /// chain above it is, and so does a chain holding only the reporter.
    ///
    /// Why this test matters: position proves which process ran the
    /// reporter, which only means something when the reporter is the hook.
    #[farhelm_testtrace::test]
    fn claude_refuses_a_non_hook_reporter_and_a_reporter_that_is_the_pane() {
        let not_hook = vec![
            corridor_link(
                11,
                "/opt/test/bin/farhelm",
                &["farhelm", "agent", "instructions"],
            ),
            corridor_link(10, CLAUDE_IMAGE, &["claude"]),
        ];
        let refusal = claude_corridor(&not_hook).expect_err("a non-hook reporter must refuse");
        assert!(refusal.contains("supported hook invocation"), "{refusal}");

        let no_argv = vec![
            corridor_link_no_argv(11, "/opt/test/bin/farhelm"),
            corridor_link(10, CLAUDE_IMAGE, &["claude"]),
        ];
        claude_corridor(&no_argv).expect_err("a reporter without argv evidence must refuse");

        let alone = vec![corridor_link(
            10,
            "/opt/test/bin/farhelm",
            &claude_hook_argv(),
        )];
        claude_corridor(&alone).expect_err("a reporter that is the pane has no emitter");
    }
}
