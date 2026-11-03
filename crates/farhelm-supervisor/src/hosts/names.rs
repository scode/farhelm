//! Host socket names: 8 random base32 characters.
//!
//! Short on purpose. A unix socket path must fit `sun_path`, which is 104
//! bytes on macOS (108 on Linux), counting the terminating NUL. A host socket
//! is `<state dir>/hosts/<name>`: six bytes for `hosts/` and eight for the
//! name, one less than `supervisor.sock`. So a host socket path is always
//! shorter than the supervisor's own socket path in the same state directory,
//! and every state directory that works today works for hosts too. Two UUIDs
//! (a session's and a tab's) would not fit.
//!
//! The name carries no meaning. What a host is for travels in its client tag
//! ([`super::tag`]), which a later supervisor reads back from the host itself,
//! so finding hosts again never depends on how they were named, and the
//! naming can change without stranding the hosts already running.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

/// Characters in a host socket name.
pub const NAME_LEN: usize = 8;

/// RFC 4648's base32 alphabet in lower case: no character a shell, a
/// filesystem or a log line treats specially.
const ALPHABET: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";

/// One host's socket name: [`NAME_LEN`] characters from [`ALPHABET`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HostName(String);

impl HostName {
    /// A fresh name from the operating system's randomness: 40 bits, so two
    /// hosts of one state directory practically never draw the same name, and
    /// a spawn that does finds the socket taken and draws again.
    pub fn random() -> io::Result<HostName> {
        let mut bytes = [0u8; 5];
        getrandom::fill(&mut bytes).map_err(|error| {
            io::Error::other(format!("reading operating-system randomness: {error}"))
        })?;
        // Five bytes are 40 bits, eight groups of five.
        let bits = bytes.iter().fold(0u64, |acc, &b| (acc << 8) | u64::from(b));
        let name = (0..NAME_LEN)
            .map(|i| ALPHABET[((bits >> (35 - 5 * i)) & 0x1f) as usize] as char)
            .collect();
        Ok(HostName(name))
    }

    /// The name as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The socket path of this name in the socket directory `hosts_dir`.
    pub fn socket_in(&self, hosts_dir: &Path) -> PathBuf {
        hosts_dir.join(&self.0)
    }
}

impl fmt::Display for HostName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hosts::ensure_hosts_dir;
    use crate::service::Supervisor;

    /// The longest path that binds as a unix socket on macOS: `sun_path` is
    /// 104 bytes there, and the path needs its terminating NUL.
    const MACOS_MAX_SOCKET_PATH: usize = 103;

    /// Names are eight characters of lower-case base32, and they differ.
    ///
    /// Why: the length is what keeps host sockets within the socket path
    /// limit, and the alphabet is what keeps names safe in a path and a log
    /// line. Spec: 200 names are each 8 characters from `a-z2-7`, and no two
    /// are equal (40 random bits make a collision in 200 draws vanishingly
    /// unlikely, so one means the randomness is broken).
    #[test]
    fn names_are_eight_random_base32_characters() {
        let mut seen = std::collections::HashSet::new();
        for _ in 0..200 {
            let name = HostName::random().unwrap();
            assert_eq!(name.as_str().len(), NAME_LEN, "{name}");
            assert!(
                name.as_str().bytes().all(|b| ALPHABET.contains(&b)),
                "{name}"
            );
            assert!(seen.insert(name.clone()), "{name} drawn twice");
        }
    }

    /// A host socket path is shorter than the supervisor's socket path in the
    /// same state directory, and binds even where that path is at macOS's
    /// limit.
    ///
    /// Why: a state directory whose `supervisor.sock` just fits must not
    /// become one whose terminals cannot start (SPEC_impl.md, "Terminal
    /// host"). Spec: in a state directory under `/tmp` padded so that
    /// `supervisor.sock` is exactly 103 bytes long, the premise holds (that
    /// path binds), and a host socket path from a real socket directory there
    /// is shorter and binds too.
    #[test]
    fn a_host_socket_path_fits_wherever_the_supervisor_socket_does() {
        // Under /tmp explicitly, not TMPDIR: the padding below needs room,
        // and a long TMPDIR would leave none.
        let base = tempfile::Builder::new()
            .prefix("fh-names")
            .tempdir_in("/tmp")
            .unwrap();
        let supervisor_suffix = Supervisor::socket_path(Path::new("")).as_os_str().len();
        let base_len = base.path().as_os_str().len();
        // `<base>/<padding>/supervisor.sock` is exactly the limit.
        let padding = MACOS_MAX_SOCKET_PATH
            .checked_sub(base_len + 1 + 1 + supervisor_suffix)
            .filter(|&padding| padding > 0)
            .expect("premise: room to pad the state directory under /tmp");
        let state = base.path().join("s".repeat(padding));
        std::fs::create_dir(&state).unwrap();
        let supervisor_socket = Supervisor::socket_path(&state);
        assert_eq!(
            supervisor_socket.as_os_str().len(),
            MACOS_MAX_SOCKET_PATH,
            "premise: the supervisor socket is at the limit"
        );
        let _supervisor = std::os::unix::net::UnixListener::bind(&supervisor_socket)
            .expect("premise: the supervisor socket binds");

        let hosts = ensure_hosts_dir(&state).unwrap();
        let host_socket = HostName::random().unwrap().socket_in(&hosts);
        assert!(
            host_socket.as_os_str().len() < supervisor_socket.as_os_str().len(),
            "{} is not shorter than {}",
            host_socket.display(),
            supervisor_socket.display()
        );
        let _host = std::os::unix::net::UnixListener::bind(&host_socket)
            .expect("the host socket binds at the limit");
    }
}
