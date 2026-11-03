//! The terminal host on its own: `terminal-host --socket <path> < child-spec`
//! (SPEC.md, "Command line and child spec"). An embedding program can run the
//! same thing from a subcommand through [`terminal_host::main_entry`].

fn main() {
    std::process::exit(terminal_host::main_entry(std::env::args_os().skip(1)));
}
