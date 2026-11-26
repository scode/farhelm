//! Farhelm's repository tooling, run as `cargo xtask <command>`.
//!
//! The only command today is `records` (see the `records` module): the tool the plan executors and the landing monitor
//! use to read and save their working logs in the shared agent-records git repository. The crate is its own cargo
//! workspace, built into `target/xtask/` by the alias in `.cargo/config.toml`, so a broken or busy root workspace never
//! stops a records push (`xtask/Cargo.toml` explains the layout).
//!
//! Every caller runs the version in its own checkout; there is no installed binary. A plan stack that changes `xtask/`
//! therefore runs its own version of the tool, including in the landing monitor's checkout while that stack lands.

mod records;

use std::process::ExitCode;

/// Top-level usage, printed for `--help` and on an unknown command.
const USAGE: &str = "\
usage: cargo xtask <command> ...

commands:
  records    read and save agent records (run `cargo xtask records --help`)
";

fn main() -> ExitCode {
    let args: Vec<String> = match std::env::args_os()
        .skip(1)
        .map(|a| a.into_string())
        .collect()
    {
        Ok(args) => args,
        Err(_) => {
            eprintln!("xtask: arguments must be valid UTF-8");
            return ExitCode::from(2);
        }
    };
    match args.first().map(String::as_str) {
        Some("records") => {
            let context = match records::Context::from_process() {
                Ok(context) => context,
                Err(failure) => {
                    eprintln!("records: {}", failure.message);
                    return ExitCode::from(failure.code);
                }
            };
            ExitCode::from(records::main(&args[1..], &context))
        }
        Some("-h" | "--help" | "help") => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprint!("xtask: unknown command `{other}`\n{USAGE}");
            ExitCode::from(2)
        }
        None => {
            eprint!("{USAGE}");
            ExitCode::from(2)
        }
    }
}
