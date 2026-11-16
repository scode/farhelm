//! The platform players for the GPUI app's status sounds: what makes the sound once the app's sound owner has decided
//! one plays (`farhelm_gui::sounds`).
//!
//! The GUI crate decides which sound plays and when, and renders each one into a WAV file in memory; it holds no
//! platform code, and its bans keep it from spawning a process or a thread. So the desktop binary, which owns the
//! platform glue, hands it a [`Player`] at startup ([`player`]). The design was decided by the migration's orchestrator
//! on 2026-11-14: no audio crate (the audio crates' ALSA and PulseAudio bindings are generated from LGPL headers, which
//! the maintainer ruled out), and on each platform the plainest way the system already offers.
//!
//! - **macOS**: AppKit's `NSSound`, made from the WAV bytes and played on the main thread, where the sound owner runs.
//!   `play` returns at once and AppKit plays the sound on its own; no thread of ours.
//! - **Linux**: PipeWire's `pw-play`, run as a child process with the path of a file to play (`pw-play <file>`), best
//!   effort. The three sounds never change, so the player writes them once, when it is made, into a private directory
//!   of its own under `XDG_RUNTIME_DIR`, and removes it when it is dropped. Files rather than the WAV on the child's standard input keep each
//!   sound to one child and nothing else: a pipe holds less than a sound, so feeding it would need a writer per sound
//!   that a child which stops reading could hold up. Reaping a child means waiting for it, which the GUI thread must
//!   not do, so one long-lived thread of this crate's own starts each child and waits for it; a sound asked for while
//!   one plays is dropped, never queued. The thread kills a child still running a few seconds after it started,
//!   because with one waiting thread a `pw-play` hung on a stalled PipeWire would otherwise silence every later sound.
//!
//! Playback is best effort everywhere, as the old app's Web Audio playback was: a sound that cannot play is lost,
//! never reported to the user, never queued or retried. Failures are logged at debug level only, since a machine
//! without PipeWire (or with no audio device) would otherwise log one line per sound for as long as the app runs.

use std::rc::Rc;

use farhelm_gui::sounds::Player;

/// The player for this platform, for the bootstrap to install; `None` where the app has none, which plays nothing.
pub(crate) fn player() -> Option<Rc<dyn Player>> {
    #[cfg(target_os = "macos")]
    let player: Option<Rc<dyn Player>> = Some(Rc::new(mac::NsSoundPlayer::default()));
    #[cfg(target_os = "linux")]
    let player: Option<Rc<dyn Player>> = Some(Rc::new(linux::PwPlay::system()));
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    let player: Option<Rc<dyn Player>> = None;
    player
}

#[cfg(target_os = "macos")]
mod mac {
    use std::cell::RefCell;

    use farhelm_gui::sounds::{Player, Sound};
    use objc2::AnyThread as _;
    use objc2::rc::Retained;
    use objc2_app_kit::NSSound;
    use objc2_foundation::NSData;

    /// Plays each sound through its own `NSSound`, made from the WAV bytes.
    ///
    /// The sounds that are still playing are kept here until they finish. AppKit's documentation does not promise that
    /// a sound whose last reference goes away while it plays keeps playing, so a sound released as soon as `play`
    /// returned might stop at once; holding it costs one small object per sound, and the finished ones are dropped the
    /// next time a sound plays.
    #[derive(Default)]
    pub(super) struct NsSoundPlayer {
        playing: RefCell<Vec<Retained<NSSound>>>,
    }

    impl Player for NsSoundPlayer {
        fn play(&self, sound: Sound, wav: &'static [u8]) {
            let mut playing = self.playing.borrow_mut();
            playing.retain(|kept| kept.isPlaying());
            let data = NSData::with_bytes(wav);
            let Some(made) = NSSound::initWithData(NSSound::alloc(), &data) else {
                tracing::debug!(?sound, "AppKit could not read the sound's WAV file");
                return;
            };
            if made.play() {
                playing.push(made);
            } else {
                tracing::debug!(?sound, "AppKit did not play the sound");
            }
        }
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use std::ffi::{OsStr, OsString};
    use std::io;
    use std::os::unix::fs::DirBuilderExt as _;
    use std::path::{Path, PathBuf};
    use std::process::{Child, Command, ExitStatus, Stdio};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::{self, Receiver, SyncSender};
    use std::time::{Duration, Instant};

    use farhelm_gui::sounds::{Player, Sound, synth};

    /// The program that plays a sound: PipeWire's player, found on the `PATH` the app was started with, so an install
    /// outside `/usr/bin` (a Nix profile, say) is found too.
    const PROGRAM: &str = "pw-play";

    /// How long a `pw-play` child may run before it is killed. The longest sound (Approval) lasts about 1.2 s; five seconds
    /// leave room for `pw-play` to connect to PipeWire on a loaded machine and still end a child that hangs (a stalled
    /// PipeWire, say) soon enough that the sounds dropped meanwhile are a few, not the rest of the session's.
    const DEADLINE: Duration = Duration::from_secs(5);

    /// How often the player's thread asks whether the child has exited. It delays only the reaping, never the sound.
    const POLL: Duration = Duration::from_millis(20);

    /// Plays each sound by running `program <file>` on the player's one thread (see the module docs).
    ///
    /// Empty when there is no usable runtime directory, the sound files could not be written or the thread could not
    /// be made: such a player plays nothing, having said why once at debug level when it was made. The program,
    /// deadline and runtime directory come from the caller so that tests run a stand-in, wait less, and keep their
    /// files in a directory of their own, without touching the process environment.
    pub(super) struct PwPlay(Option<Playing>);

    /// A working player's half on the GUI thread.
    struct Playing {
        requests: SyncSender<Sound>,
        /// Set by `play` when it hands the thread a sound and cleared by the thread once that sound's child is reaped,
        /// so while it is set a sound is waiting or playing and a new one is dropped.
        busy: Arc<AtomicBool>,
        /// Held for its drop alone, which removes the directory: the thread reads the files by path.
        _files: SoundFiles,
    }

    impl PwPlay {
        /// The app's player: `pw-play` from the `PATH`, the default deadline, and its files under `XDG_RUNTIME_DIR`.
        pub(super) fn system() -> Self {
            let runtime_dir = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from);
            PwPlay::new(PROGRAM.into(), DEADLINE, runtime_dir)
        }

        /// A player of `program`, its sound files in a new directory in `runtime_dir`.
        ///
        /// The runtime directory is the only place the files go, with no fallback to the temporary directory:
        /// `pw-play` reaches PipeWire through the same per-user session that provides `XDG_RUNTIME_DIR`, so without
        /// one there is nothing to play to, and the session empties it at logout, which clears a directory a crashed
        /// app left behind. A relative path counts as unset, as the XDG base directory specification says; it would
        /// also make `pw-play`'s argument depend on its working directory.
        fn new(program: OsString, deadline: Duration, runtime_dir: Option<PathBuf>) -> Self {
            let Some(runtime_dir) = runtime_dir.filter(|dir| dir.is_absolute()) else {
                tracing::debug!("XDG_RUNTIME_DIR is unset or not absolute; no sound will play");
                return PwPlay(None);
            };
            let files = match SoundFiles::write(&runtime_dir) {
                Ok(files) => files,
                Err(error) => {
                    tracing::debug!(%error, "the sound files could not be written; no sound will play");
                    return PwPlay(None);
                }
            };
            // One place in the channel is enough: `busy` lets a sound in only while nothing is waiting or playing.
            let (requests, received) = mpsc::sync_channel(1);
            let busy = Arc::new(AtomicBool::new(false));
            let (thread_busy, dir) = (busy.clone(), files.dir.clone());
            let started = std::thread::Builder::new()
                .name("farhelm-sound".into())
                .spawn(move || serve(&program, &dir, deadline, received, &thread_busy));
            match started {
                Ok(_) => PwPlay(Some(Playing {
                    requests,
                    busy,
                    _files: files,
                })),
                Err(error) => {
                    tracing::debug!(%error, "no thread to play sounds on; no sound will play");
                    PwPlay(None)
                }
            }
        }
    }

    impl Player for PwPlay {
        /// Hand `sound` to the player's thread, or drop it while another is playing. `wav` goes unused: the file
        /// written for `sound` holds the same bytes, since both are [`synth::wav`]'s.
        fn play(&self, sound: Sound, _wav: &'static [u8]) {
            let Some(playing) = &self.0 else { return };
            if playing.busy.swap(true, Ordering::AcqRel) {
                tracing::debug!(?sound, "a sound is already playing; this one is dropped");
                return;
            }
            // The channel cannot be full while `busy` was clear, so only a thread that has ended refuses the sound.
            // `busy` then stays set and every later sound is dropped, which is all a player without a thread can do.
            if let Err(error) = playing.requests.try_send(sound) {
                tracing::debug!(?sound, %error, "the sound thread is gone");
            }
        }
    }

    /// A directory holding one WAV file per [`Sound`], removed (best effort) when this is dropped.
    struct SoundFiles {
        dir: PathBuf,
    }

    impl SoundFiles {
        /// Write the sound files into a new directory in `base`. The directory's name is new for every player, since
        /// several Farhelm processes may run as one user, and only the user may enter it (mode 0700), so nobody else
        /// can swap a file in it before `pw-play` opens it. Creating it fails rather than reusing one already there.
        fn write(base: &Path) -> io::Result<SoundFiles> {
            let dir = base.join(format!("farhelm-sounds-{}", uuid::Uuid::new_v4()));
            std::fs::DirBuilder::new().mode(0o700).create(&dir)?;
            // Made before the files are written, so a failed write removes the directory again.
            let files = SoundFiles { dir };
            for sound in Sound::PRIORITY {
                std::fs::write(sound_path(&files.dir, sound), synth::wav(sound))?;
            }
            Ok(files)
        }
    }

    impl Drop for SoundFiles {
        /// A child still playing keeps its file open, so removing it under the child is harmless on Linux.
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    /// Where `sound`'s file is in the player's directory `dir`.
    fn sound_path(dir: &Path, sound: Sound) -> PathBuf {
        dir.join(match sound {
            Sound::Approval => "approval.wav",
            Sound::Waiting => "waiting.wav",
            Sound::TurnFinished => "turn-finished.wav",
        })
    }

    /// The player's thread: play each sound handed over until the player is dropped, clearing `busy` after each.
    /// A child killed at the deadline logs as ended by signal 9.
    fn serve(
        program: &OsStr,
        dir: &Path,
        deadline: Duration,
        requests: Receiver<Sound>,
        busy: &AtomicBool,
    ) {
        for sound in requests {
            match play_file(program, &sound_path(dir, sound), deadline) {
                Ok(status) if status.success() => {}
                Ok(status) => tracing::debug!(?sound, %status, "pw-play did not play the sound"),
                Err(error) => tracing::debug!(?sound, %error, "pw-play could not be run"),
            }
            busy.store(false, Ordering::Release);
        }
    }

    /// Run `program file`, and wait for it to exit or kill it at `deadline`. Blocks until then, so it runs on the
    /// player's own thread, never the GUI's. Once started, the child is always waited for, so no zombie is left.
    ///
    /// The child reads nothing from us and its output and errors go nowhere: `pw-play` prints nothing a user needs,
    /// and a pipe nobody reads could fill and stall it; the exit status is what the log keeps. With no pipe to it,
    /// nothing but the child itself can hold the wait up, and the deadline bounds that.
    ///
    /// NOTE: The deadline kills the child alone. A grandchild it started would be left running, which `pw-play` is not
    /// expected to do. And the bound holds only while the app runs: a `pw-play` hung on a stalled PipeWire when the app
    /// quits is left running.
    fn play_file(program: &OsStr, file: &Path, deadline: Duration) -> io::Result<ExitStatus> {
        let mut child = Command::new(program)
            .arg(file)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        reap(&mut child, deadline)
    }

    /// Wait for `child` until `deadline` has passed, then kill it and wait again.
    fn reap(child: &mut Child, deadline: Duration) -> io::Result<ExitStatus> {
        let until = Instant::now() + deadline;
        loop {
            match child.try_wait() {
                Ok(Some(status)) => return Ok(status),
                Ok(None) if Instant::now() < until => std::thread::sleep(POLL),
                // Past the deadline, or the child cannot be asked: end it either way, so it never runs on unwatched.
                _ => break,
            }
        }
        // The kill's result is ignored: the child is not reaped yet, so its pid is still its own and the kill succeeds
        // even if the child has just exited. What can fail here is the wait after a failed `try_wait`, and that error
        // is returned.
        let _ = child.kill();
        child.wait()
    }

    #[cfg(test)]
    mod tests {
        use std::os::unix::fs::PermissionsExt as _;

        use super::*;

        /// `pw-play` plays the file named by its argument, so each sound must reach the child as exactly one argument
        /// naming a file that holds that sound's WAV, in the player's private directory. `sh` stands in for `pw-play`,
        /// which a build machine may not have. Specified: the player's directory is new in the runtime directory it
        /// was given, with mode 0700; for each sound, the stand-in sees one argument, a file in that directory whose
        /// bytes are the sound's [`synth::wav`].
        #[farhelm_testtrace::test]
        fn the_child_gets_the_sounds_file_as_its_only_argument() {
            let dir = tempfile::tempdir().unwrap();
            let args = dir.path().join("args");
            let body = format!("printf '%s %s\\n' \"$#\" \"$1\" >> '{}'", args.display());
            let player = stand_in_player(stand_in(dir.path(), "player", &body), dir.path());
            let own = player.0.as_ref().unwrap()._files.dir.clone();
            assert_eq!(own.parent(), Some(dir.path()));
            let mode = std::fs::metadata(&own).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o700, "{mode:o}");

            for (played, sound) in Sound::PRIORITY.into_iter().enumerate() {
                player.play(sound, synth::wav(sound));
                idle(&player);
                let text = std::fs::read_to_string(&args).unwrap();
                let lines: Vec<&str> = text.lines().collect();
                assert_eq!(lines.len(), played + 1, "{lines:?}");
                let (count, path) = lines[played].split_once(' ').unwrap();
                assert_eq!(count, "1", "{sound:?}: {}", lines[played]);
                let path = Path::new(path);
                assert_eq!(path.parent(), Some(own.as_path()), "{sound:?}");
                let bytes = std::fs::read(path).unwrap();
                assert!(
                    bytes == synth::wav(sound),
                    "{sound:?}: {path:?} holds other bytes"
                );
            }
        }

        /// The sound owner calls `play` on the GUI thread, so `play` must hand the sound over and return, never wait
        /// for the child; and one thread plays one sound at a time, while a sound that waited for the one before it
        /// would play late and out of step with what it announces, so a sound asked for meanwhile is dropped.
        /// Specified: with a stand-in that runs for a minute, `play` returns within a second and the child starts and
        /// keeps the player busy; every further sound meanwhile starts no child, at once or after the first one ends.
        #[farhelm_testtrace::test]
        fn play_returns_at_once_and_drops_sounds_while_a_child_runs() {
            let dir = tempfile::tempdir().unwrap();
            let starts = dir.path().join("starts");
            let body = format!("echo $$ >> '{}'\nexec sleep 60", starts.display());
            let player = stand_in_player(stand_in(dir.path(), "player", &body), dir.path());

            let before = Instant::now();
            player.play(Sound::Approval, synth::wav(Sound::Approval));
            let took = before.elapsed();
            assert!(took < Duration::from_secs(1), "play took {took:?}");
            assert!(
                soon(|| pids(&starts).len() == 1),
                "the stand-in never started"
            );
            assert!(busy(&player), "still playing");
            for sound in Sound::PRIORITY {
                player.play(sound, synth::wav(sound));
            }
            // Once the player is idle nothing is waiting either: a sound let in keeps it busy until its child is
            // reaped, so a dropped sound that had been kept would show here as a second start.
            end_stand_ins(&starts, &player);
            assert_eq!(pids(&starts).len(), 1, "a dropped sound never starts later");
        }

        /// With one thread waiting for each child, a `pw-play` hung on a stalled PipeWire would silence every later
        /// sound unless it is killed. Specified: a stand-in that would run for a minute is ended once the deadline
        /// has passed and reaped (its pid gone, not a zombie), and the next sound then starts a child of its own.
        #[farhelm_testtrace::test]
        fn a_hung_child_is_killed_at_the_deadline_and_the_next_sound_plays() {
            let dir = tempfile::tempdir().unwrap();
            let starts = dir.path().join("starts");
            // Only Approval hangs; any other sound exits at once.
            let body = format!(
                "echo $$ >> '{}'\ncase \"$1\" in */approval.wav) exec sleep 60;; esac",
                starts.display()
            );
            let program = stand_in(dir.path(), "player", &body);
            let deadline = Duration::from_secs(1);
            let player = PwPlay::new(program.into(), deadline, Some(dir.path().to_owned()));

            let before = Instant::now();
            player.play(Sound::Approval, synth::wav(Sound::Approval));
            assert!(
                soon(|| pids(&starts).len() == 1),
                "the stand-in never started"
            );
            let hung = pids(&starts)[0];
            idle(&player);
            let took = before.elapsed();
            assert!(took >= deadline, "ended too early: {took:?}");
            // SAFETY: kill with signal 0 only asks whether the pid exists; no pointer is passed. A zombie still exists.
            let exists = unsafe { libc::kill(hung, 0) } == 0;
            assert!(!exists, "the hung stand-in {hung} was not reaped");

            player.play(Sound::Waiting, synth::wav(Sound::Waiting));
            assert!(
                soon(|| pids(&starts).len() == 2),
                "the next sound never played"
            );
            idle(&player);
        }

        /// `pw-play` may well be missing (no PipeWire, a minimal desktop), and a session may have no usable runtime
        /// directory; the sound is best effort, so either must play nothing without a panic. Specified: a sound
        /// played through a missing program leaves the player idle again; with the runtime directory unset, relative,
        /// or a plain file (which refuses a directory in it even to root, unlike a read-only directory), the player is
        /// empty and a sound starts no child.
        #[farhelm_testtrace::test]
        fn a_missing_player_or_directory_plays_nothing() {
            let dir = tempfile::tempdir().unwrap();
            let player = stand_in_player(dir.path().join("no-such-player"), dir.path());
            player.play(Sound::Approval, synth::wav(Sound::Approval));
            idle(&player);

            let not_a_dir = dir.path().join("not-a-directory");
            std::fs::write(&not_a_dir, "").unwrap();
            let starts = dir.path().join("starts");
            let body = format!("echo $$ >> '{}'", starts.display());
            let program = stand_in(dir.path(), "player", &body);
            for runtime_dir in [None, Some(PathBuf::from("relative")), Some(not_a_dir)] {
                let silent = PwPlay::new(program.clone().into(), DEADLINE, runtime_dir.clone());
                assert!(
                    silent.0.is_none(),
                    "{runtime_dir:?}: no directory, so no thread"
                );
                silent.play(Sound::Approval, synth::wav(Sound::Approval));
            }
            // With no thread there is nothing that could start a child later.
            assert!(pids(&starts).is_empty());
        }

        /// The sound files must not outlive the player, and two Farhelm processes of one user must not share them.
        /// Specified: two players make two directories, and each is gone once its player is dropped.
        #[farhelm_testtrace::test]
        fn each_player_has_its_own_directory_removed_on_drop() {
            let dir = tempfile::tempdir().unwrap();
            let entries = || std::fs::read_dir(dir.path()).unwrap().count();
            let never_run = dir.path().join("never-run");

            let first = stand_in_player(never_run.clone(), dir.path());
            let second = stand_in_player(never_run, dir.path());
            assert_eq!(entries(), 2, "one directory per player");
            drop(first);
            assert_eq!(entries(), 1);
            drop(second);
            assert_eq!(entries(), 0);
        }

        /// A stand-in for `pw-play` at `dir/name`: a `/bin/sh` script running `body`, runnable when returned.
        ///
        /// Running an executable just written races `ETXTBSY` against any other thread's fork, which may still hold
        /// the descriptor that wrote it, so the script is run with `--warmup` (which exits before the body) until it
        /// runs, as the supervisor's host spawn tests do.
        fn stand_in(dir: &Path, name: &str, body: &str) -> PathBuf {
            use std::os::unix::fs::OpenOptionsExt as _;
            let path = dir.join(name);
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o755)
                .open(&path)
                .unwrap();
            let script = format!("#!/bin/sh\ncase \"$1\" in --warmup) exit 0;; esac\n{body}\n");
            std::io::Write::write_all(&mut file, script.as_bytes()).unwrap();
            drop(file);
            for _ in 0..200 {
                match Command::new(&path).arg("--warmup").output() {
                    Ok(_) => return path,
                    Err(error) if error.raw_os_error() == Some(libc::ETXTBSY) => {
                        // sleep-ok: retry only ETXTBSY while an inherited writer closes; a successful exec is the fixture boundary.
                        std::thread::sleep(Duration::from_millis(5))
                    }
                    Err(error) => panic!("stand-in {name} is not runnable: {error}"),
                }
            }
            panic!("stand-in {name} stayed busy");
        }

        /// A player of `program` with its files in a new directory in `runtime_dir`, and a deadline no test reaches:
        /// the tests end their stand-ins themselves ([`end_stand_ins`]), so that a loaded machine cannot make a
        /// deadline fire under a "still running" assertion. The deadline itself is covered by
        /// `a_hung_child_is_killed_at_the_deadline_and_the_next_sound_plays`.
        fn stand_in_player(program: PathBuf, runtime_dir: &Path) -> PwPlay {
            let deadline = Duration::from_secs(120);
            PwPlay::new(program.into(), deadline, Some(runtime_dir.to_owned()))
        }

        /// Whether `player` has a sound waiting or playing.
        fn busy(player: &PwPlay) -> bool {
            let playing = player.0.as_ref().expect("the player has its thread");
            playing.busy.load(Ordering::Acquire)
        }

        /// Require `player` to become idle soon. The thread clears `busy` only after it waited for the sound's child,
        /// so this also shows no stand-in is left running or unreaped.
        fn idle(player: &PwPlay) {
            assert!(soon(|| !busy(player)), "the player is still busy");
        }

        /// The pids the stand-ins wrote to `starts` as they started, one per line.
        fn pids(starts: &Path) -> Vec<libc::pid_t> {
            let text = std::fs::read_to_string(starts).unwrap_or_default();
            text.lines().map(|line| line.parse().unwrap()).collect()
        }

        /// Kill the stand-ins whose pids are in `starts`, then require `player` to become idle. Killing by pid is safe:
        /// the player has not waited for these children yet, so their pids cannot have been reused.
        fn end_stand_ins(starts: &Path, player: &PwPlay) {
            for pid in pids(starts) {
                // SAFETY: kill takes no pointers, and `pid` is an unreaped child of this process.
                assert_eq!(unsafe { libc::kill(pid, libc::SIGKILL) }, 0, "kill {pid}");
            }
            idle(player);
        }

        /// Whether `done` holds within 30 s, asked every 20 ms. Far longer than any stand-in here needs, so that only
        /// a real hang runs it out on a loaded machine.
        fn soon(mut done: impl FnMut() -> bool) -> bool {
            let until = Instant::now() + Duration::from_secs(30);
            while !done() {
                if Instant::now() >= until {
                    return false;
                }
                // sleep-ok: polling interval for a condition bounded by the 30 s above.
                std::thread::sleep(Duration::from_millis(20));
            }
            true
        }
    }
}
