//! OMP's foreground corridor: its installation-descriptor evidence (Bun
//! bundle, source tree, or compiled target), the launcher and trampoline
//! shapes a launch may use, and the corridor that attributes a report to the
//! one OMP runtime that launch installed.

use super::*;

/// OMP's installation-descriptor evidence, matched over walked chain links.
///
/// The installed `omp` command is a script: the kernel runs its `bun`
/// shebang, so the live runtime image is the Bun binary and the entry point
/// is the bundle path in program-argument position. The compiled target is
/// a native `omp` image instead. Both shapes are recognized by exact
/// bundle/entry suffixes, never by basename alone.
pub(super) const OMP_DIST_ENTRY_SUFFIX: &[u8] =
    b"node_modules/@oh-my-pi/pi-coding-agent/dist/cli.js";
pub(super) const OMP_SRC_ENTRY_SUFFIX: &[u8] = b"packages/coding-agent/src/cli.ts";
pub(super) const OMP_PACKAGE: &[u8] = b"@oh-my-pi/pi-coding-agent";
/// OMP's selected command under npm/npx: the package's own bin, spelled
/// exactly as the launcher resolves it. Anything else after the option
/// boundary is a foreign command the package merely makes available.
pub(super) const OMP_BIN: &[u8] = b"omp";

/// The raw image basename, over bytes the caller already captured, with the
/// procfs atomic-update suffix stripped the way the emitter check strips
/// it. Shared by every OMP image test so no caller re-derives it.
pub(super) fn omp_image_basename(exe: &[u8]) -> &[u8] {
    let name = exe.rsplit(|byte| *byte == b'/').next().unwrap_or_default();
    name.strip_suffix(b" (deleted)").unwrap_or(name)
}

/// Whether the image is a Bun interpreter: the only interpreter whose
/// execution of OMP is a supported shape. Node execution is unverified and
/// refused elsewhere, never inferred from a `.js` suffix.
pub(super) fn is_bun_image(exe: &[u8]) -> bool {
    omp_image_basename(exe) == b"bun"
}

/// Whether the image is a Node interpreter: observed only to refuse with a
/// diagnostic naming the mechanism rather than a generic unclassified
/// intermediary.
pub(super) fn is_node_image(exe: &[u8]) -> bool {
    matches!(omp_image_basename(exe), b"node" | b"nodejs")
}

/// Whether the image is a compiled OMP target: the native binary the
/// pinned `build-binary.ts` emits as `dist/omp` (or a `dist/omp-<id>`
/// cross build). A script named `omp` never matches: scripts resolve to
/// their interpreter's image before this comparison runs.
pub(super) fn is_compiled_omp_image(exe: &[u8]) -> bool {
    let name = omp_image_basename(exe);
    name == b"omp" || name.starts_with(b"omp-")
}

/// Whether raw argv spells the selected OMP entry point: the installed
/// bundle or the explicitly selected source-tree `src/cli.ts`, matched as
/// a path suffix so a same-named file elsewhere is not evidence.
pub(super) fn is_omp_bundle_entry(arg: &[u8]) -> bool {
    arg.ends_with(OMP_DIST_ENTRY_SUFFIX) || arg.ends_with(OMP_SRC_ENTRY_SUFFIX)
}

/// Whether a Bun launcher link spells a supported package launch: `bun x`
/// or `bunx` selecting exactly OMP's package. Only the head is
/// constrained; trailing tokens are OMP's own arguments and the runtime
/// grammar below judges them. Any other spelling — flags before the
/// subcommand, `--package` forms, package scripts — is not an observed
/// shape and refuses.
pub(super) fn is_omp_bun_launcher(argv: &[Vec<u8>]) -> bool {
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
pub(super) fn is_npm_launcher_level_token(token: &[u8]) -> bool {
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
pub(super) fn is_omp_npm_launcher(argv: &[Vec<u8>]) -> bool {
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
pub(super) fn is_omp_shell_trampoline(exe: &[u8], argv: &[Vec<u8>], expected: &[&[u8]]) -> bool {
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

/// The launcher predicate plus the trampoline target spellings one OMP
/// launch program admits above its runtime.
pub(super) type OmpLauncherRules<'a> = (fn(&[Vec<u8>]) -> bool, &'a [&'a [u8]]);

/// OMP's instance of the restrictive corridor, over an already-walked
/// chain: the reporter (first link, never itself a runtime candidate) must
/// be the supported hook invocation; exactly one link must be the launched
/// runtime's descriptor (Bun plus the selected entry, or the compiled
/// target with TUI grammar); the pane anchor (last link) is accepted by
/// position, except that an `omp` launch refuses a Bun or Node pane that is
/// not the emitter; links below the runtime must be narrow hook
/// trampolines, and links above it must be recognized launchers or
/// transparent shell trampolines for the launch's own program. Any
/// additional session-hosting runtime of any kind and every unclassified
/// intermediary refuses.
///
/// `program` is the durable launch's program classification: it selects
/// which launcher spellings may appear above the runtime, so a chain whose
/// live shape contradicts its launch refuses rather than being re-explained.
///
/// Pure over the chain so the shapes are unit-testable without live
/// processes; admission supplies the report's recorded chain, anchored at the
/// session's pane.
pub(crate) fn omp_corridor(
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
    // The pane anchor is accepted by position, but for a launch of the
    // installed `omp` command nothing launches the runtime: the pane either
    // IS the runtime or a shell above it. A Bun or Node pane that is not
    // the emitter is therefore a runtime the search above could not
    // classify (its arguments unreadable, say, past the argv budget), and
    // the emitter it found is a nested OMP below it. Admitting that would
    // let the nested conversation's report stand for the session's own.
    if matches!(program, OmpLaunchProgram::Omp) {
        let pane_index = chain.len() - 1;
        let pane = &chain[pane_index];
        if pane_index != emitter_index && (is_bun_image(&pane.exe) || is_node_image(&pane.exe)) {
            return Err(
                "the pane runs a Bun or Node process that is not the reporting OMP runtime"
                    .to_string(),
            );
        }
    }
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
                    && (is_omp_bundle_entry(&argv[1]) || omp_resolved_entry_matches(link, &argv[1]))
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
pub(super) fn no_omp_launcher(_argv: &[Vec<u8>]) -> bool {
    false
}

/// Whether one non-reporter link is an OMP runtime image: Bun (the entry
/// check needs its argv, so an argv-less Bun link is not a candidate here;
/// downstream it refuses as an unclassified intermediary, or as the pane of
/// an installed-`omp` launch, and is accepted as any other launch's pane)
/// or the compiled target.
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
pub(super) fn is_omp_runtime_link(link: &ChainLink) -> bool {
    if is_compiled_omp_image(&link.exe) {
        return true;
    }
    if !is_bun_image(&link.exe) {
        return false;
    }
    link.argv.as_deref().is_some_and(|argv| {
        argv.len() >= 2
            && omp_image_basename(&argv[0]) == b"bun"
            && (is_omp_bundle_entry(&argv[1]) || omp_resolved_entry_matches(link, &argv[1]))
    })
}

/// Whether the canonical path behind one entry spelling is the selected
/// OMP bundle or source tree. `None` on any failure — undecodable bytes,
/// a relative spelling with no recorded working directory, a dangling
/// link — so callers treat unresolvable spellings as the raw bytes they
/// already refused.
pub(super) fn omp_resolved_entry_matches(link: &ChainLink, entry: &[u8]) -> bool {
    resolved_omp_entry(link.cwd.as_deref(), entry)
        .is_some_and(|resolved| is_omp_bundle_entry(&resolved))
}

/// The canonical path behind one entry spelling, or `None` when it
/// cannot be established. A relative spelling resolves against `cwd`, the
/// SUBJECT's working directory as the chain recorded it — never this
/// process's, and never a live read, so a chain keeps its meaning after
/// the process it describes has exited. Blocking filesystem I/O: callers
/// run on the attribution thread, never an async context.
pub(super) fn resolved_omp_entry(cwd: Option<&[u8]>, entry: &[u8]) -> Option<Vec<u8>> {
    use std::os::unix::ffi::OsStrExt;
    let entry = std::str::from_utf8(entry).ok()?;
    let path = std::path::Path::new(entry);
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::path::Path::new(std::ffi::OsStr::from_bytes(cwd?)).join(path)
    };
    std::fs::canonicalize(absolute)
        .ok()
        .map(|canonical| os_bytes(&canonical))
}

/// The raw bytes behind one path, for byte-level matching over kernel
/// evidence. Unix-only, like every other reader in this module.
pub(super) fn os_bytes(path: &std::path::Path) -> Vec<u8> {
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
pub(super) fn omp_runtime_tui_grammar(link: &ChainLink) -> Result<(), String> {
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
