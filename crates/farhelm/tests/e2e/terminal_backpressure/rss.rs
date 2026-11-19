//! Sample one process incarnation without revisiting its numeric PID path.
//!
//! Linux keeps an open /proc/PID descriptor tied to that process after exit;
//! it cannot silently name a later process that reuses the PID. smaps_rollup
//! avoids statm's asynchronous RSS accounting and reports explicit KiB units,
//! so neither delayed counters nor an assumed page size enter the allowance.
//! See https://www.kernel.org/doc/html/latest/filesystems/proc.html.

use std::fs::{File, OpenOptions};
use std::io::{self, Read};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::fs::OpenOptionsExt;

const PROC_READ_LIMIT: u64 = 16 * 1024;

/// A measurement handle, not authority to signal or wait for this process.
/// Callers obtain the PID from their own fixture, then keep this directory
/// open across every sample. Missing/dead/unreadable processes fail explicitly.
pub(super) struct ProcessRss {
    directory: File,
    pub(super) pid: u32,
    /// Kernel start time identifies the sample series in retained diagnostics.
    pub(super) start_time_ticks: u64,
}

impl ProcessRss {
    /// Bind the fixture's reported process before the measurement window.
    /// Opening the directory once prevents later samples from following PID reuse.
    pub(super) fn bind(pid: u32) -> io::Result<Self> {
        let directory = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
            .open(format!("/proc/{pid}"))?;
        let mut process = Self {
            directory,
            pid,
            start_time_ticks: 0,
        };
        process.start_time_ticks = parse_start_time(&process.read(c"stat")?, pid)?;
        Ok(process)
    }

    /// Read resident bytes from one rollup scan of this bound process.
    /// No fallback to statm: changing metrics on failure could manufacture a pass.
    pub(super) fn bytes(&self) -> io::Result<u64> {
        parse_rss(&self.read(c"smaps_rollup")?)
    }

    /// Read one fixed proc entry through the retained directory with bounded
    /// allocation. Procfs IO and scheduler stalls still depend on the kernel;
    /// the outer test runner supplies the overall execution deadline.
    fn read(&self, name: &std::ffi::CStr) -> io::Result<String> {
        // SAFETY: both the directory and NUL-terminated entry name remain live
        // through openat. A successful result is a new descriptor owned below.
        let fd = unsafe {
            libc::openat(
                self.directory.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: openat returned this new descriptor; no other owner exists.
        let file = unsafe { File::from_raw_fd(fd) };
        let mut text = String::new();
        file.take(PROC_READ_LIMIT + 1).read_to_string(&mut text)?;
        if text.len() as u64 > PROC_READ_LIMIT {
            return Err(invalid("proc entry exceeded the sample bound"));
        }
        Ok(text)
    }
}

/// Require exactly one resident total with the kernel's explicit KiB unit.
/// Missing, duplicated, malformed and overflowing values are invalid evidence.
fn parse_rss(text: &str) -> io::Result<u64> {
    let mut rss = None;
    for line in text.lines() {
        let Some(value) = line.strip_prefix("Rss:") else {
            continue;
        };
        let mut fields = value.split_whitespace();
        let kib = fields
            .next()
            .and_then(|value| value.parse::<u64>().ok())
            .ok_or_else(|| invalid("invalid resident total"))?;
        if rss.is_some() || fields.next() != Some("kB") || fields.next().is_some() {
            return Err(invalid("ambiguous resident total or unit"));
        }
        rss = Some(
            kib.checked_mul(1024)
                .ok_or_else(|| invalid("resident total overflow"))?,
        );
    }
    rss.ok_or_else(|| invalid("resident total missing"))
}

/// stat's parenthesized command can contain spaces and closing parentheses.
/// Field 22 is the twentieth field after that command; never split the whole row.
fn parse_start_time(text: &str, expected_pid: u32) -> io::Result<u64> {
    let (pid, _) = text
        .split_once(' ')
        .ok_or_else(|| invalid("stat PID missing"))?;
    if pid.parse::<u32>().ok() != Some(expected_pid) {
        return Err(invalid("stat PID does not match the fixture"));
    }
    text.rsplit_once(')')
        .and_then(|(_, fields)| fields.split_whitespace().nth(19))
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| invalid("stat start time missing"))
}

/// Data-shape errors remain distinguishable from process exit or IO denial.
fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Metric parsing must not turn unavailable or differently scaled evidence
    /// into a plausible memory sample; unrelated rollup fields are harmless.
    #[farhelm_testtrace::test]
    fn resident_total_requires_one_bounded_value_in_kib() {
        assert_eq!(
            parse_rss("header\nRss: 17 kB\nPss: 2 kB\n").unwrap(),
            17 * 1024
        );
        for text in [
            "",
            "Pss: 2 kB",
            "Rss: -1 kB",
            "Rss: 4 bytes",
            "Rss: 4 kB extra",
            "Rss: 4 kB\nRss: 4 kB",
            "Rss: 18446744073709551615 kB",
        ] {
            assert!(parse_rss(text).is_err(), "accepted {text:?}");
        }
    }

    /// Process names cannot shift the start-time field or hide a mismatched PID.
    #[farhelm_testtrace::test]
    fn process_identity_handles_parentheses_in_command_names() {
        let fields = format!("S {} 456 0", ["0"; 18].join(" "));
        let stat = format!("123 (name with ) parentheses) {fields}");
        assert_eq!(parse_start_time(&stat, 123).unwrap(), 456);
        assert!(parse_start_time(&stat, 124).is_err());
        assert!(parse_start_time("123 (short) S 0", 123).is_err());
    }
}
