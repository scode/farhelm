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
//! - **Linux**: PipeWire's `pw-play`, run as a child process with the WAV on its standard input (`pw-play -`), best
//!   effort. Writing a sound of about 100 KB into a pipe would block the GUI thread once the pipe's buffer fills, and
//!   reaping the child means waiting for it, so each sound gets a thread of this crate's own that spawns the child,
//!   feeds it from a second thread (a child that stops reading then cannot hold up the reaping), and kills it if it is
//!   still running a few seconds after the sound should have ended. At most a few sounds play at once and the rest are
//!   dropped, so a stalled PipeWire costs a bounded number of threads and children, not one more per sound.
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
    use std::io::{self, Write as _};
    use std::process::{Child, Command, ExitStatus, Stdio};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    use farhelm_gui::sounds::{Player, Sound};

    /// The program that plays a sound: PipeWire's player, found on the `PATH` the app was started with, so an install
    /// outside `/usr/bin` (a Nix profile, say) is found too.
    const PROGRAM: &str = "pw-play";

    /// How long a `pw-play` child may run before it is killed. The longest sound (Approval) lasts about 1.2 s; five seconds
    /// leave room for `pw-play` to connect to PipeWire on a loaded machine and still end a child that hangs (a stalled
    /// PipeWire, say) soon enough that the cap below is not held for long.
    const DEADLINE: Duration = Duration::from_secs(5);

    /// How many sounds may play at once; past this a sound is dropped, never queued. A sound lasts about a second and
    /// the owner asks for at most one per observation, so more than a few at once are either stuck or too close
    /// together to tell apart, and starting more children would only add to them.
    const MAX_PLAYING: usize = 4;

    /// How often the reaper asks whether the child has exited. It delays only the reaping, never the sound.
    const POLL: Duration = Duration::from_millis(20);

    /// The sounds playing in this process, against [`MAX_PLAYING`].
    static PLAYING: AtomicUsize = AtomicUsize::new(0);

    /// Plays each sound through a `program -` child of its own, on a thread of its own (see the module docs).
    ///
    /// The program, deadline and count are fields so that tests can run a stand-in, wait less, and count their own
    /// sounds apart from other tests' in the same process.
    pub(super) struct PwPlay {
        program: OsString,
        deadline: Duration,
        playing: &'static AtomicUsize,
    }

    impl PwPlay {
        /// The app's player: `pw-play` from the `PATH`, the default deadline and the process-wide count.
        pub(super) fn system() -> Self {
            PwPlay {
                program: PROGRAM.into(),
                deadline: DEADLINE,
                playing: &PLAYING,
            }
        }
    }

    impl Player for PwPlay {
        fn play(&self, sound: Sound, wav: &'static [u8]) {
            let Some(slot) = Slot::take(self.playing) else {
                tracing::debug!(
                    ?sound,
                    "{MAX_PLAYING} sounds are already playing; this one is dropped"
                );
                return;
            };
            let (program, deadline) = (self.program.clone(), self.deadline);
            // A failed spawn drops the closure, and the slot with it.
            let started = std::thread::Builder::new()
                .name("farhelm-sound".into())
                .spawn(move || {
                    let _slot = slot;
                    match play_with(&program, wav, deadline) {
                        Ok(Ended::Exited(status)) if status.success() => {}
                        Ok(Ended::Exited(status)) => {
                            tracing::debug!(?sound, %status, "pw-play did not play the sound")
                        }
                        Ok(Ended::Killed(status)) => {
                            tracing::debug!(?sound, %status, "pw-play ran past its deadline, killed")
                        }
                        Err(error) => tracing::debug!(?sound, %error, "pw-play could not be fed or run"),
                    }
                });
            if let Err(error) = started {
                tracing::debug!(?sound, %error, "no thread to play the sound on");
            }
        }
    }

    /// One of the [`MAX_PLAYING`] places, held for as long as a sound's thread runs. Dropping it gives the place back,
    /// however the thread ends, a panic included.
    struct Slot(&'static AtomicUsize);

    impl Slot {
        /// A place in `count`, or `None` when all of them are taken.
        fn take(count: &'static AtomicUsize) -> Option<Slot> {
            count
                .try_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                    (n < MAX_PLAYING).then_some(n + 1)
                })
                .ok()
                .map(|_| Slot(count))
        }
    }

    impl Drop for Slot {
        fn drop(&mut self) {
            self.0.fetch_sub(1, Ordering::AcqRel);
        }
    }

    /// How a child that [`play_with`] started ended; either way it has been waited for, so no zombie is left.
    #[derive(Debug)]
    enum Ended {
        /// It exited on its own.
        Exited(ExitStatus),
        /// It was still running at the deadline and was killed.
        Killed(ExitStatus),
    }

    /// Run `program -` with `wav` on its standard input, and wait for it to exit or kill it at `deadline`. Blocks until
    /// then, so it runs on the player's own thread, never the GUI's.
    ///
    /// The child's output goes nowhere: `pw-play` prints nothing a user needs, and a pipe nobody reads could fill and
    /// stall it. Its error output goes nowhere too, for the same reason; the exit status is what the log keeps. The WAV
    /// is written from a thread of its own, so a child that stops reading (it failed to open the audio device, say)
    /// cannot hold up the reaping. Killing the child closes the pipe, so a write still blocked then fails with a broken
    /// pipe and its thread ends; it fails rather than killing the app because Rust binaries ignore SIGPIPE. A failed
    /// write counts only when the child exited zero, since otherwise the exit says more. Once the child is spawned it
    /// is always reaped, even when no thread can be made to feed it.
    ///
    /// NOTE: That relies on the child alone holding the pipe. A grandchild that inherited it would keep the write
    /// blocked, and this thread with it (and its place under the cap), until the grandchild exits. `pw-play` is not
    /// expected to start one, and the tests' stand-ins `exec` their last command so that they do not either. The
    /// bounds hold only while the app runs: a `pw-play` hung on a stalled PipeWire when the app quits is left running.
    fn play_with(program: &OsStr, wav: &[u8], deadline: Duration) -> io::Result<Ended> {
        let mut child = Command::new(program)
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        let input = child.stdin.take();
        let (ended, written) = std::thread::scope(|scope| {
            // The input is dropped when the write ends, which closes the pipe: `pw-play` reads to the end of it. A
            // failed spawn drops it too, so the child sees its input end and the reaping below still runs.
            let writer = std::thread::Builder::new()
                .name("farhelm-sound-feed".into())
                .spawn_scoped(scope, move || match input {
                    Some(mut input) => input.write_all(wav),
                    None => Ok(()),
                });
            let ended = reap(&mut child, deadline);
            let written = match writer {
                Ok(writer) => writer
                    .join()
                    .unwrap_or_else(|_| Err(io::Error::other("the writer panicked"))),
                Err(error) => Err(error),
            };
            (ended, written)
        });
        match ended? {
            Ended::Exited(status) if status.success() => written.map(|()| Ended::Exited(status)),
            ended => Ok(ended),
        }
    }

    /// Wait for `child` until `deadline` has passed, then kill it and wait again.
    fn reap(child: &mut Child, deadline: Duration) -> io::Result<Ended> {
        use std::os::unix::process::ExitStatusExt as _;
        let until = Instant::now() + deadline;
        loop {
            match child.try_wait() {
                Ok(Some(status)) => return Ok(Ended::Exited(status)),
                Ok(None) if Instant::now() < until => std::thread::sleep(POLL),
                // Past the deadline, or the child cannot be asked: end it either way, so it never runs on unwatched.
                _ => break,
            }
        }
        // The kill's result is ignored: the child is not reaped yet, so its pid is still its own and the kill succeeds
        // even if the child has just exited. What can fail here is the wait after a failed `try_wait`, and that error
        // is returned.
        let _ = child.kill();
        let status = child.wait()?;
        // A child that exited on its own between the last poll and the kill was not killed.
        Ok(if status.signal() == Some(libc::SIGKILL) {
            Ended::Killed(status)
        } else {
            Ended::Exited(status)
        })
    }

    #[cfg(test)]
    mod tests {
        use std::os::unix::process::ExitStatusExt as _;
        use std::path::{Path, PathBuf};

        use farhelm_gui::sounds::synth;

        use super::*;

        /// The WAV must reach the child whole on its standard input, after the `-` argument that tells `pw-play` to
        /// read it from there, and the child must be waited for, so its exit status is known and no zombie is left.
        /// `sh` stands in for `pw-play`, which a build machine may not have. Specified: a child that checks its one
        /// argument is `-` and that its input is exactly the bytes written exits zero; one that exits 3 reports 3.
        #[farhelm_testtrace::test]
        fn the_wav_reaches_the_child_on_its_input_after_a_dash() {
            let wav = synth::wav(Sound::TurnFinished);
            let dir = tempfile::tempdir().unwrap();
            let expected = dir.path().join("expected.wav");
            std::fs::write(&expected, wav).unwrap();
            let body = format!(
                "[ \"$#\" = 1 ] && [ \"$1\" = - ] && cat | cmp -s - '{}'",
                expected.display()
            );
            let checker = stand_in(dir.path(), "player", &body);
            let ended = play_with(checker.as_os_str(), wav, DEADLINE).unwrap();
            assert!(
                matches!(ended, Ended::Exited(status) if status.success()),
                "{ended:?}"
            );

            let failing = stand_in(dir.path(), "failing", "cat >/dev/null\nexit 3");
            let ended = play_with(failing.as_os_str(), wav, DEADLINE).unwrap();
            assert!(
                matches!(ended, Ended::Exited(status) if status.code() == Some(3)),
                "{ended:?}"
            );
        }

        /// The sound owner calls `play` on the GUI thread, so `play` must hand everything to a thread of its own and
        /// return, even when the child never reads its input. Specified: with a stand-in that ignores its input and
        /// runs for a minute, `play` returns within a second, and the child is still running after that.
        #[farhelm_testtrace::test]
        fn play_returns_at_once_while_the_child_ignores_its_input() {
            static PLAYING: AtomicUsize = AtomicUsize::new(0);
            let dir = tempfile::tempdir().unwrap();
            let starts = dir.path().join("starts");
            let body = format!("echo $$ >> '{}'\nexec sleep 60", starts.display());
            let player = stand_in_player(stand_in(dir.path(), "player", &body), &PLAYING);
            let wav = synth::wav(Sound::Approval);

            let before = Instant::now();
            player.play(Sound::Approval, wav);
            let took = before.elapsed();
            assert!(took < Duration::from_secs(1), "play took {took:?}");
            assert!(
                soon(|| pids(&starts).len() == 1),
                "the stand-in never started"
            );
            assert_eq!(PLAYING.load(Ordering::Acquire), 1, "still running");
            end_stand_ins(&starts, &PLAYING);
        }

        /// A child that hangs (PipeWire stalled, say) and never reads its input must not hold its thread, its pipe and
        /// itself for good. The WAV is larger than a new pipe's buffer (checked, since the buffer's size depends on the
        /// page size), so the write blocks too. Specified: such a child is killed once the deadline passes and waited
        /// for, its status reporting SIGKILL.
        #[farhelm_testtrace::test]
        fn a_hung_child_is_killed_at_the_deadline() {
            use std::os::fd::AsRawFd as _;
            let dir = tempfile::tempdir().unwrap();
            let hung = stand_in(dir.path(), "hung", "exec sleep 60");
            let wav = synth::wav(Sound::Approval);
            let (_reader, writer) = std::io::pipe().unwrap();
            // SAFETY: F_GETPIPE_SZ only reads the size of a pipe this test owns; no pointer is passed.
            let pipe_size = unsafe { libc::fcntl(writer.as_raw_fd(), libc::F_GETPIPE_SZ) };
            let fills = usize::try_from(pipe_size).is_ok_and(|size| wav.len() > size);
            assert!(
                fills,
                "premise: {} bytes fill a {pipe_size}-byte pipe",
                wav.len()
            );
            let deadline = Duration::from_secs(1);

            let (sender, receiver) = std::sync::mpsc::channel();
            let before = Instant::now();
            std::thread::spawn(move || sender.send(play_with(hung.as_os_str(), wav, deadline)));
            let ended = receiver
                .recv_timeout(Duration::from_secs(30))
                .expect("play_with returns soon after the deadline")
                .unwrap();
            assert!(before.elapsed() >= deadline, "killed too early");
            assert!(
                matches!(ended, Ended::Killed(status) if status.signal() == Some(libc::SIGKILL)),
                "{ended:?}"
            );
        }

        /// The cap is what bounds the threads and children a stalled PipeWire can pile up. Specified: with
        /// [`MAX_PLAYING`] stand-ins still running, further sounds start no child, at once or later.
        #[farhelm_testtrace::test]
        fn sounds_past_the_cap_are_dropped_while_earlier_ones_play() {
            static PLAYING: AtomicUsize = AtomicUsize::new(0);
            let dir = tempfile::tempdir().unwrap();
            let starts = dir.path().join("starts");
            let body = format!("echo $$ >> '{}'\nexec sleep 60", starts.display());
            let player = stand_in_player(stand_in(dir.path(), "player", &body), &PLAYING);
            let wav = synth::wav(Sound::Waiting);
            let started = || pids(&starts).len();

            for _ in 0..MAX_PLAYING + 2 {
                player.play(Sound::Waiting, wav);
            }
            assert_eq!(PLAYING.load(Ordering::Acquire), MAX_PLAYING);
            assert!(soon(|| started() == MAX_PLAYING), "{} started", started());
            assert_eq!(PLAYING.load(Ordering::Acquire), MAX_PLAYING, "running");
            end_stand_ins(&starts, &PLAYING);
            assert_eq!(started(), MAX_PLAYING, "a dropped sound never starts later");
        }

        /// `pw-play` may well be missing (no PipeWire, a minimal desktop), and the sound is best effort, so a missing
        /// program must come back as an error the thread logs, and the sound must give its place under the cap back.
        /// Specified: running a program that does not exist gives `NotFound`, not a panic; a sound played through it
        /// leaves nothing counted as playing (which shows the place comes back, not that its thread did not panic).
        #[farhelm_testtrace::test]
        fn a_missing_player_plays_nothing_and_leaves_nothing_running() {
            static PLAYING: AtomicUsize = AtomicUsize::new(0);
            let dir = tempfile::tempdir().unwrap();
            let missing = dir.path().join("no-such-player");
            let error = play_with(missing.as_os_str(), b"RIFF", DEADLINE)
                .expect_err("nothing runs a missing program");
            assert_eq!(error.kind(), io::ErrorKind::NotFound);

            stand_in_player(missing, &PLAYING).play(Sound::Approval, synth::wav(Sound::Approval));
            drained(&PLAYING);
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

        /// A player of `program` that counts its sounds in the test's own `playing`, with a deadline no test reaches:
        /// the tests end their stand-ins themselves ([`end_stand_ins`]), so that a loaded machine cannot make a
        /// deadline fire under a "still running" assertion. The deadline itself is covered by
        /// `a_hung_child_is_killed_at_the_deadline`.
        fn stand_in_player(program: PathBuf, playing: &'static AtomicUsize) -> PwPlay {
            let deadline = Duration::from_secs(120);
            PwPlay {
                program: program.into(),
                deadline,
                playing,
            }
        }

        /// The pids the stand-ins wrote to `starts` as they started, one per line.
        fn pids(starts: &Path) -> Vec<libc::pid_t> {
            let text = std::fs::read_to_string(starts).unwrap_or_default();
            text.lines().map(|line| line.parse().unwrap()).collect()
        }

        /// Kill the stand-ins whose pids are in `starts`, then require their sounds to drain from `count`. Killing by
        /// pid is safe: the player has not waited for these children yet, so their pids cannot have been reused.
        fn end_stand_ins(starts: &Path, count: &AtomicUsize) {
            for pid in pids(starts) {
                // SAFETY: kill takes no pointers, and `pid` is an unreaped child of this process.
                assert_eq!(unsafe { libc::kill(pid, libc::SIGKILL) }, 0, "kill {pid}");
            }
            drained(count);
        }

        /// Require every sound counted in `count` to end soon. A sound's place is given back only when its thread
        /// ends, after its child was waited for, so this also shows no stand-in is left running.
        fn drained(count: &AtomicUsize) {
            let ended = soon(|| count.load(Ordering::Acquire) == 0);
            assert!(ended, "{} still playing", count.load(Ordering::Acquire));
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
