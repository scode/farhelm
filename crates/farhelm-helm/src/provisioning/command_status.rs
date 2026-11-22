//! Status records for provisioning commands. Some SSH servers lose the remote
//! exit code. A per-command tag distinguishes the wrapper's record from other
//! stderr, while stdout keeps its existing data and protocol framing.

use std::ops::Range;

const MARKER: &str = "farhelm-command-status-v1";

/// A fresh tag for each wrapper. It separates records from incidental command
/// and transport output, including output from another provisioning command.
#[derive(Clone, Debug)]
pub(super) struct Tag(String);

impl Tag {
    pub(super) fn new() -> Self {
        Self(uuid::Uuid::new_v4().simple().to_string())
    }

    /// Keep the reporter in a parent shell so the script can exit, set an EXIT
    /// trap, or exec another program and still have its outcome reported. The
    /// extra newline gives the record its own line after unterminated stderr.
    /// The wrapper reserves its own exit 255 for transport failure. The
    /// record preserves a child's 255 even when the wrapper exits 254.
    pub(super) fn wrap(&self, script: &str) -> String {
        let script = crate::ssh::shell_quote(script);
        let tag = &self.0;
        format!(
            "sh -c {script}\nstatus=$?; printf '\\n%s %s %s\\n' {MARKER} {tag} \"$status\" >&2; \
             if [ \"$status\" = 255 ]; then exit 254; fi; exit \"$status\""
        )
    }

    /// Read one complete line, excluding its newline. Only the invocation's
    /// marker and tag followed by an unsigned byte in decimal count as a record.
    pub(super) fn parse_line(&self, line: &[u8]) -> Option<i32> {
        let line = line.strip_prefix(MARKER.as_bytes())?.strip_prefix(b" ")?;
        let code = line.strip_prefix(self.0.as_bytes())?.strip_prefix(b" ")?;
        if code.is_empty() || code.len() > 3 || !code.iter().all(u8::is_ascii_digit) {
            return None;
        }
        Some(i32::from(
            std::str::from_utf8(code).ok()?.parse::<u8>().ok()?,
        ))
    }

    /// Find the final complete record and the bytes it contributed to stderr.
    /// The range includes the wrapper's preceding newline, even if the script
    /// had already ended stderr with its own newline.
    pub(super) fn find_record(&self, stderr: &[u8]) -> Option<(i32, Range<usize>)> {
        let mut end = 0;
        let mut found = None;
        for line in stderr.split_inclusive(|byte| *byte == b'\n') {
            let start = end;
            end += line.len();
            if let Some(line) = line.strip_suffix(b"\n")
                && let Some(code) = self.parse_line(line)
            {
                found = Some((code, start.saturating_sub(1)..end));
            }
        }
        found
    }

    /// Maximum bytes added to stderr, including both newlines, two spaces,
    /// and the widest possible decimal status.
    pub(super) fn max_record_bytes(&self) -> usize {
        MARKER.len() + self.0.len() + 7
    }

    /// Recover the tag from the final wrapper suffix on a test's lowered SSH
    /// or Podman script argument. Shell quoting does not change the hex word.
    /// Stand-ins that replace the payload can therefore keep using this wrapper.
    #[cfg(test)]
    pub(super) fn in_command(command: &tokio::process::Command) -> Option<Self> {
        let script = command.as_std().get_args().last()?.to_str()?;
        let (_, suffix) = script.rsplit_once(&format!(" {MARKER} "))?;
        let (tag, _) = suffix.split_once(' ')?;
        if tag.len() != 32 || !tag.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        Some(Self(tag.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tag correlates a single command, and only a complete decimal record
    /// can supply its status. Unrelated lines remain ordinary diagnostics.
    #[test]
    fn only_complete_matching_records_supply_a_status() {
        let tag = Tag("chosen".into());
        let record = |body: &str| format!("{MARKER} {body}");
        for invalid in [
            "other 0",
            "chosen",
            "chosen 256",
            "chosen +1",
            "chosen -1",
            "chosen 1 ",
        ] {
            assert_eq!(
                tag.parse_line(record(invalid).as_bytes()),
                None,
                "{invalid}"
            );
        }
        assert_eq!(tag.parse_line(b"chosen"), None);
        assert_eq!(tag.parse_line(record("chosen 255").as_bytes()), Some(255));
        assert!(tag.find_record(record("chosen 0").as_bytes()).is_none());
    }

    /// Remove the wrapper's separator and record without changing either a
    /// command's own final newline or stderr that arrived after the record.
    #[test]
    fn stripping_removes_only_the_record_and_the_wrappers_newline() {
        let tag = Tag("chosen".into());
        for diagnostic in [
            "",
            "problem",
            "problem\n",
            "farhelm-command-status-v1 other 4\n",
        ] {
            let mut stderr = format!("{diagnostic}\n{MARKER} chosen 76\nssh trailer\n");
            let (code, range) = tag.find_record(stderr.as_bytes()).unwrap();
            stderr.replace_range(range, "");
            assert_eq!(code, 76);
            assert_eq!(stderr, format!("{diagnostic}ssh trailer\n"));
        }
    }
}
