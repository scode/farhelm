//! This crate's unit test binary as a terminal host program, so that its
//! tests run their sessions in real terminal hosts, as a shipped supervisor
//! does, without a `farhelm` binary to start them from. glibc Linux only.
//!
//! A supervisor starts a host as `<program> internal terminal-host --socket
//! <path>` (`hosts::spawn`). In the Rust e2e suite `<program>` is the built
//! `farhelm`; a unit test binary has none, and libtest would read those
//! arguments as test filters. So the test binary answers that command line
//! itself, before libtest's `main` runs: an initializer the loader calls at
//! process start (`entry::ENTRY`) checks the arguments and, when they are a
//! host start, runs the host crate's own entry point
//! ([`terminal_host::main_entry`], the same one `farhelm internal
//! terminal-host` runs) and exits with its status. Any other command line,
//! the test runs included, passes through untouched.
//!
//! Before `main` is the only place this can run. The host's first stage
//! forks and its second stage goes on running as the host, so the entry
//! point must be called from a single-threaded process, and libtest starts a
//! thread per test before any test code runs. At initializer time the
//! process is a fresh exec of the test binary with no thread but its own.
//! The host is then a separate process running the same host code, with
//! the command line the supervisor's verified signals expect (`--socket
//! <path>` last), so what a test does to it (Stop, Delete, a Restart, a
//! supervisor dropped and started again) reaches a real host. The one
//! difference from `farhelm internal terminal-host` is that the entry point
//! runs before Rust's own runtime setup in `main` (SIGPIPE still has its
//! default disposition, for one). That does not change the host: its second
//! stage installs its own signal dispositions and sends with `MSG_NOSIGNAL`,
//! and its first stage only reads the spec and reports on stderr and its
//! readiness pipe.
//!
//! The initializer is registered for glibc Linux, whose loader passes
//! `argc` and `argv` to initializers, and which is where these tests run.
//! It is not registered anywhere else (musl calls initializers with no
//! arguments; macOS's would be untested). There [`unit_test_host_program`]
//! still names the test binary, and every host start through it fails:
//! libtest refuses `--socket` as an unknown option, which the spawn reports
//! as a refused start, so a unit test that needs a host fails by name.

use std::path::PathBuf;

/// The program a unit test's supervisor starts terminal hosts from
/// (`SupervisorSeams::host_program`): this test binary, which runs a host
/// when started with a host's command line (`entry::ENTRY`).
pub(crate) fn unit_test_host_program() -> PathBuf {
    std::env::current_exe().expect("the unit test binary's own path")
}

/// The loader initializer, registered on glibc Linux only (module docs).
#[cfg(all(target_os = "linux", target_env = "gnu"))]
mod entry {
    use std::ffi::{CStr, OsStr, OsString, c_char, c_int};
    use std::os::unix::ffi::OsStrExt as _;

    /// The arguments after the program name that make this binary a host:
    /// the first stage's command line `hosts::spawn` runs, minus the socket
    /// path.
    const HOST_PREFIX: [&str; 3] = ["internal", "terminal-host", "--socket"];

    /// The initializer's signature, as glibc's loader calls it.
    type Initializer = extern "C" fn(c_int, *const *const c_char, *const *const c_char);

    /// Registered with the loader, which calls it once before `main`.
    #[used]
    #[unsafe(link_section = ".init_array")]
    static ENTRY: Initializer = host_entry;

    /// Run as a host when the arguments are a host start, and return at once
    /// otherwise.
    ///
    /// Reads `argv` as the loader passed it rather than through
    /// `std::env::args`, whose own capture of the arguments is another
    /// initializer that may not have run yet.
    extern "C" fn host_entry(argc: c_int, argv: *const *const c_char, _envp: *const *const c_char) {
        if argv.is_null() || argc < 1 {
            return;
        }
        let args: Vec<OsString> = (1..argc as usize)
            .map(|index| {
                // SAFETY: the loader passes `argc` valid, NUL-terminated
                // argument strings in `argv`, which live for the whole
                // process.
                let arg = unsafe { CStr::from_ptr(*argv.add(index)) };
                OsStr::from_bytes(arg.to_bytes()).to_os_string()
            })
            .collect();
        let is_host_start = args.len() == HOST_PREFIX.len() + 1
            && args.iter().zip(HOST_PREFIX).all(|(arg, want)| arg == want);
        if !is_host_start {
            return;
        }
        std::process::exit(terminal_host::main_entry(args.into_iter().skip(2)));
    }
}
