//! Command-line parsing for `records`, hand-rolled to keep the tool's build small.
//!
//! The grammar is `[--config <path>] <command> ...` with three commands. Everything wrong with the command line,
//! including a malformed record name, is a usage error (exit 2) found here, before the config is read.

use std::path::PathBuf;

use super::Failure;
use super::names::RecordName;

/// `records --help` output.
pub const USAGE: &str = "\
usage: cargo xtask records [--config <path>] <command> ...

commands:
  path <name>                  sync this slot's clone, print the record's directory (creating it)
  push <name>... -m <message>  commit exactly the named records' directories and push them, without force
  backup --dest <dir>          fetch every branch of the records repo into the bare repo <dir>

record names: plan/<slug>, executor, landing, round/<n>
The default config is ~/.fh-records/config. Exit status: 0 success, 1 failed or refused, 2 usage error.
";

/// A parsed command line.
#[derive(Debug, PartialEq, Eq)]
pub enum Parsed {
    Help,
    Run(Invocation),
}

/// A command to run and the config it should read (`None`: the default path).
#[derive(Debug, PartialEq, Eq)]
pub struct Invocation {
    pub config: Option<PathBuf>,
    pub command: Command,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Path {
        name: RecordName,
    },
    Push {
        names: Vec<RecordName>,
        message: String,
    },
    Backup {
        dest: PathBuf,
    },
}

/// Parses the arguments that follow `records`.
pub fn parse(args: &[String]) -> Result<Parsed, Failure> {
    let mut rest = args;
    let mut config = None;
    // Global options come before the command word only, so a record name or message can never be mistaken for one.
    loop {
        match rest.first().map(String::as_str) {
            Some("-h" | "--help") => return Ok(Parsed::Help),
            Some("--config") => {
                let value = rest.get(1).ok_or_else(|| usage("--config needs a path"))?;
                config = Some(PathBuf::from(value));
                rest = &rest[2..];
            }
            Some(arg) if arg.starts_with("--config=") => {
                config = Some(PathBuf::from(&arg["--config=".len()..]));
                rest = &rest[1..];
            }
            _ => break,
        }
    }
    let (command, rest) = rest.split_first().ok_or_else(|| usage("missing command"))?;
    let is_help = |a: &String| a == "-h" || a == "--help";
    // `push` finds `--help` itself, because after `-m` the same word is a message, not a request for help.
    if command != "push" && rest.iter().any(is_help) {
        return Ok(Parsed::Help);
    }
    let command = match command.as_str() {
        "path" => match rest {
            [name] => Command::Path {
                name: RecordName::parse(name)?,
            },
            _ => return Err(usage("path takes exactly one record name")),
        },
        "push" => match parse_push(rest)? {
            Some(command) => command,
            None => return Ok(Parsed::Help),
        },
        "backup" => parse_backup(rest)?,
        other => return Err(usage(&format!("unknown command `{other}`"))),
    };
    Ok(Parsed::Run(Invocation { config, command }))
}

/// `push <name>... -m <message>`; `-m` may come before, between or after the names, once. `None` means a standalone
/// `-h`/`--help` asked for usage.
fn parse_push(args: &[String]) -> Result<Option<Command>, Failure> {
    let mut names = Vec::new();
    let mut message = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(None),
            "-m" | "--message" => {
                let value = iter.next().ok_or_else(|| usage("-m needs a message"))?;
                if message.replace(value.clone()).is_some() {
                    return Err(usage("push takes one -m message"));
                }
            }
            flag if flag.starts_with('-') => {
                return Err(usage(&format!("unknown push option `{flag}`")));
            }
            name => names.push(RecordName::parse(name)?),
        }
    }
    let message = message.ok_or_else(|| usage("push needs -m <message>"))?;
    if message.trim().is_empty() {
        return Err(usage("the push message must not be empty"));
    }
    if names.is_empty() {
        return Err(usage("push needs at least one record name"));
    }
    Ok(Some(Command::Push { names, message }))
}

/// `backup --dest <dir>`. The directory must be absolute: backup runs from a scheduler whose working directory is
/// nobody's choice, so a relative path would put the backup wherever that happens to be.
fn parse_backup(args: &[String]) -> Result<Command, Failure> {
    let dest = match args {
        [flag, dest] if flag == "--dest" => PathBuf::from(dest),
        [arg] if arg.starts_with("--dest=") => PathBuf::from(&arg["--dest=".len()..]),
        _ => return Err(usage("backup takes exactly --dest <dir>")),
    };
    if !dest.is_absolute() {
        return Err(usage(&format!(
            "backup --dest needs an absolute path, not `{}`",
            dest.display()
        )));
    }
    Ok(Command::Backup { dest })
}

/// A usage failure whose message ends with a pointer to `--help`.
fn usage(message: &str) -> Failure {
    Failure::usage(format!("{message} (see `cargo xtask records --help`)"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    fn run(list: &[&str]) -> Invocation {
        match parse(&args(list)).unwrap() {
            Parsed::Run(invocation) => invocation,
            Parsed::Help => panic!("unexpected help for {list:?}"),
        }
    }

    /// The contract's command forms parse into the commands they name, with `--config` in either spelling.
    #[test]
    fn contract_forms_parse() {
        assert_eq!(
            run(&["--config", "/c", "path", "plan/x"]),
            Invocation {
                config: Some(PathBuf::from("/c")),
                command: Command::Path {
                    name: RecordName::Plan("x".into())
                },
            }
        );
        assert_eq!(
            run(&["--config=/c", "push", "executor", "plan/x", "-m", "msg"]).command,
            Command::Push {
                names: vec![RecordName::Executor, RecordName::Plan("x".into())],
                message: "msg".into()
            }
        );
        assert_eq!(
            run(&["push", "-m", "msg", "landing"]).command,
            Command::Push {
                names: vec![RecordName::Landing],
                message: "msg".into()
            }
        );
        assert_eq!(
            run(&["backup", "--dest", "/b"]).command,
            Command::Backup {
                dest: PathBuf::from("/b")
            }
        );
        assert_eq!(run(&["path", "landing"]).config, None);
    }

    /// Every malformed command line is a usage error (exit 2), so a caller can tell "you called me wrong" from "the
    /// operation failed".
    #[test]
    fn malformed_command_lines_are_usage_errors() {
        for list in [
            &[][..],
            &["--config"],
            &["frobnicate"],
            &["path"],
            &["path", "plan/x", "plan/y"],
            &["path", "plan/X"],
            &["push", "plan/x"],
            &["push", "-m", "msg"],
            &["push", "plan/x", "-m"],
            &["push", "plan/x", "-m", "  "],
            &["push", "plan/x", "-m", "a", "-m", "b"],
            &["push", "plan/x", "--force", "-m", "a"],
            &["backup"],
            &["backup", "--dest"],
            &["backup", "/b"],
            &["backup", "--dest", "relative/dir"],
            &["backup", "--dest=relative"],
            &["path", "--config", "/c", "plan/x"],
        ] {
            assert_eq!(parse(&args(list)).unwrap_err().code, 2, "{list:?}");
        }
    }

    /// `--help` works anywhere, without a valid command line around it.
    #[test]
    fn help_is_recognized() {
        for list in [
            &["--help"][..],
            &["-h"],
            &["--config", "/c", "--help"],
            &["push", "--help"],
            &["push", "plan/x", "-h", "-m", "msg"],
            &["path", "--help"],
        ] {
            assert_eq!(parse(&args(list)).unwrap(), Parsed::Help, "{list:?}");
        }
    }

    /// After `-m`, `--help` is the message: an agent may well log a push whose message is a flag-like word, and turning
    /// it into a help request would silently skip the push.
    #[test]
    fn help_after_m_is_a_message() {
        assert_eq!(
            run(&["push", "plan/x", "-m", "--help"]).command,
            Command::Push {
                names: vec![RecordName::Plan("x".into())],
                message: "--help".into()
            }
        );
    }
}
