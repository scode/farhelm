//! `cargo xtask records`: the agent records tool.
//!
//! The plan executors and the landing monitor keep their working logs (a plan's log, an executor slot's log, the
//! monitor's log, a landing round's text files) in one shared git repository, the agent-records repo, instead of in
//! files beside their checkouts. This tool is the only way those unattended flows touch that repo: it gives each
//! caller a stable directory to write in and a push that cannot clobber anyone else's records. Attended flows
//! (triage, releases, deflake, run-all-tests) use plain git on their own clones instead; see the credential helper,
//! `scripts/records-git-credential.py`.
//!
//! # Invocation
//!
//! `cargo xtask records [--config <path>] <command> ...`, from the root of a Farhelm checkout (the alias's relative
//! paths only resolve there; the tool itself finds its checkout from any subdirectory). Exit status 0 is success, 1
//! means the operation failed or was refused (stderr says which, and the clone is left consistent), 2 is a usage error
//! (bad arguments or a malformed record name). The default config path is `~/.fh-records/config`, and `~` in config
//! values means the process's `HOME`.
//!
//! # Config
//!
//! A TOML file (`config` module): `remote` (any git URL or local path), an optional `token_file` (absent for a local
//! bare repo), an optional `clones_dir` (default `~/.fh-records/clones`), an optional `[artifacts]` table that only
//! `backup` reads, and `[[slot]]` entries pairing a slot name with a checkout path.
//!
//! A slot is one writer: an executor checkout (`exec-1`, `exec-2`, ...) or the landing monitor's checkout (`landing`).
//! The tool's slot is the one whose `checkout` is the nearest checkout containing the working directory. Slots whose
//! checkout does not exist are normal (the config pre-seeds more executor slots than usually run), and the tool never
//! iterates over slots to act on them: a slot's clone is created only by that slot's own first `path` or `push`.
//!
//! # Record names
//!
//! | Name          | Directory in the repo       | Who may push            |
//! | ------------- | --------------------------- | ----------------------- |
//! | `plan/<slug>` | `farhelm/plans/<slug>/`     | executor slots          |
//! | `executor`    | `farhelm/executors/<slot>/` | that executor slot only |
//! | `landing`     | `farhelm/landing/`          | the landing slot        |
//! | `round/<n>`   | `farhelm/rounds/<n>/`       | the landing slot        |
//!
//! Callers never see the `farhelm/` prefix; `executor` from the landing slot is an error (`names` module).
//!
//! # Commands
//!
//! - `path <name>` brings the slot's clone (`<clones_dir>/<slot>/`, cloned on first use) up to date with the remote
//!   and prints the record directory's absolute path, creating the directory. Any valid name may be read.
//! - `push <name>... -m <message>` brings the clone up to date the same way, then builds one commit on the remote head
//!   holding exactly the named records' directories as they are in the working tree, and pushes it to `main` without
//!   force. It prints `pushed <commit id>`, or `nothing to push` when the named directories match the remote.
//! - `backup --dest <dir>` mints a short-lived read token through Cloudflare's REST API and fetches every branch into
//!   the bare repo at `<dir>`, refusing any non-fast-forward update. It needs no slot (`backup` module).
//!
//! Bringing a clone up to date updates only the files the remote changed and keeps every other local edit. When a
//! remote-changed file also has a local edit, ownership decides (`clone` module): a working tree that already holds
//! the remote's version is no conflict (that is how a push the server accepted but the client saw fail heals); a path
//! whose remote changes all carry this slot's own stamp keeps the local file, since a slot cannot conflict with itself
//! (that is how the same failed push heals when the agent kept writing; it relies on slot names being unique across
//! every host sharing the repository); an overlap inside a record being pushed refuses the push and changes nothing
//! (two writers on one record); any other overlap is a stale leftover from a claim this slot no longer holds, so the
//! remote wins after the local file is copied to `.git/records-set-aside/<UTC timestamp>/<path>` and named on stderr.
//!
//! Failure messages never quote the configured remote (it is replaced with `<records remote>`, and its host with
//! `<records host>`) or the Cloudflare account id, because git quotes the URL in its errors and the messages end up in
//! logs and reports.
//!
//! # Credentials
//!
//! With `token_file` set, git child processes get `Authorization: Bearer <token>` as an HTTP header through their
//! environment, never on a command line, in the clone's config or in output (`git` module).

mod backup;
mod cli;
mod clone;
mod config;
mod error;
mod git;
mod names;

use std::fs;
use std::path::{Path, PathBuf};

use cli::{Command, Parsed};
pub use error::Failure;
use git::Git;
use names::RecordName;

/// What the tool reads from its process: the working directory (which picks the slot and the checkout) and `HOME`
/// (which `~` in the config and the default config path expand against).
///
/// Everything below `main` takes these as values rather than reading the process environment, so unit tests can supply
/// them directly and integration tests set them only on the child process they spawn.
pub struct Context {
    pub cwd: PathBuf,
    pub home: Option<PathBuf>,
}

impl Context {
    /// Captures the real working directory and `HOME`.
    pub fn from_process() -> Result<Self, Failure> {
        let cwd = std::env::current_dir()
            .map_err(|e| Failure::failed(format!("cannot read the working directory: {e}")))?;
        let home = std::env::var_os("HOME")
            .filter(|h| !h.is_empty())
            .map(PathBuf::from);
        Ok(Self { cwd, home })
    }
}

/// Runs `records` with the arguments after the word `records`, reports any failure on stderr, and returns the exit
/// status the contract assigns (0, 1 or 2).
pub fn main(args: &[String], context: &Context) -> u8 {
    match run(args, context) {
        Ok(()) => 0,
        Err(failure) => {
            eprintln!("records: {}", failure.message);
            failure.code
        }
    }
}

/// Parses the arguments and dispatches to the command. Argument and record-name errors are found here, before any
/// config is read or any clone is touched.
fn run(args: &[String], context: &Context) -> Result<(), Failure> {
    let invocation = match cli::parse(args)? {
        Parsed::Help => {
            print!("{}", cli::USAGE);
            return Ok(());
        }
        Parsed::Run(invocation) => invocation,
    };
    let config_path = match invocation.config {
        Some(path) => path,
        None => default_config_path(context)?,
    };
    match invocation.command {
        Command::Path { name } => path_command(context, &config_path, &name),
        Command::Push { names, message } => push_command(context, &config_path, &names, &message),
        Command::Backup { dest } => backup::run_and_notify(context, &config_path, &dest),
    }
}

/// `~/.fh-records/config`, the host-wide config every slot on a host shares.
fn default_config_path(context: &Context) -> Result<PathBuf, Failure> {
    let home = context.home.as_ref().ok_or_else(|| {
        Failure::failed("HOME is not set, so there is no default config path; pass --config <path>")
    })?;
    Ok(home.join(".fh-records").join("config"))
}

/// `path <name>`: syncs the slot's clone and prints the record directory, creating it.
fn path_command(context: &Context, config_path: &Path, name: &RecordName) -> Result<(), Failure> {
    let config = config::load(config_path, context.home.as_deref())?;
    path_with(context, config_path, &config, name).map_err(|f| f.redact_remote(&config.remote))
}

fn path_with(
    context: &Context,
    config_path: &Path,
    config: &config::Config,
    name: &RecordName,
) -> Result<(), Failure> {
    let slot = config.slot_for(&context.cwd, config_path)?;
    let directory = name.directory(&slot.name)?;
    let git = Git::new(config.read_token()?);
    let clone = clone::SlotClone::open(&git, &config.clones_dir, &slot.name, &config.remote)?;
    clone.sync(&[])?;
    let absolute = clone.root().join(&directory);
    fs::create_dir_all(&absolute)
        .map_err(|e| Failure::failed(format!("cannot create {}: {e}", absolute.display())))?;
    println!("{}", absolute.display());
    Ok(())
}

/// `push <name>... -m <message>`: checks every name against the slot's permissions first, then commits and pushes.
fn push_command(
    context: &Context,
    config_path: &Path,
    names: &[RecordName],
    message: &str,
) -> Result<(), Failure> {
    let config = config::load(config_path, context.home.as_deref())?;
    push_with(context, config_path, &config, names, message)
        .map_err(|f| f.redact_remote(&config.remote))
}

fn push_with(
    context: &Context,
    config_path: &Path,
    config: &config::Config,
    names: &[RecordName],
    message: &str,
) -> Result<(), Failure> {
    let slot = config.slot_for(&context.cwd, config_path)?;
    // The permission check comes before the token is read or the clone is opened, so a refused push has no effect
    // at all, not even creating the slot's clone.
    let mut directories = Vec::new();
    for name in names {
        name.check_push(&slot.name)?;
        let directory = name.directory(&slot.name)?;
        if !directories.contains(&directory) {
            directories.push(directory);
        }
    }
    let git = Git::new(config.read_token()?);
    let clone = clone::SlotClone::open(&git, &config.clones_dir, &slot.name, &config.remote)?;
    match clone.push(&directories, message, &slot.name)? {
        clone::PushOutcome::NothingToPush => println!("nothing to push"),
        clone::PushOutcome::Pushed(commit) => println!("pushed {commit}"),
    }
    Ok(())
}
