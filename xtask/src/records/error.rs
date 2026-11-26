//! The one error type: a message for stderr and the exit status the contract gives it.

/// A failed or refused operation, or a usage error.
///
/// The code is the process exit status: 1 for "the operation failed or was refused" and 2 for "the arguments were
/// wrong". Callers (the landing monitor, the executors' instructions) branch on the status, never on the message, so
/// the message is free to name paths and git's own words.
#[derive(Debug)]
pub struct Failure {
    pub code: u8,
    pub message: String,
}

impl Failure {
    /// Exit status 1: the operation failed or was refused.
    pub fn failed(message: impl Into<String>) -> Self {
        Self {
            code: 1,
            message: message.into(),
        }
    }

    /// Exit status 2: the command line itself was wrong.
    pub fn usage(message: impl Into<String>) -> Self {
        Self {
            code: 2,
            message: message.into(),
        }
    }

    /// Replaces every occurrence of the configured remote in the message with `<records remote>`.
    ///
    /// Git quotes the remote URL in its errors and push reports, and an Artifacts URL carries the Cloudflare account
    /// id. Failure messages end up in agents' logs and reports, which may be published, so the URL never appears in
    /// one. A remote that is not a URL (a local bare repo in tests) is replaced the same way.
    ///
    /// The remote's host (between `://` and the next `/`) is replaced too, as `<records host>`: network errors name the
    /// bare host (`Could not resolve host: <account id>.artifacts.cloudflare.net`) without the rest of the URL.
    pub fn redact_remote(self, remote: &str) -> Self {
        if remote.is_empty() {
            return self;
        }
        let mut message = self.message.replace(remote, "<records remote>");
        if let Some((_, rest)) = remote.split_once("://") {
            let host = rest.split('/').next().unwrap_or_default();
            if !host.is_empty() {
                message = message.replace(host, "<records host>");
            }
        }
        Self {
            code: self.code,
            message,
        }
    }

    /// Prefixes the message with what the tool was doing, keeping the exit status.
    pub fn context(self, doing: &str) -> Self {
        Self {
            code: self.code,
            message: format!("{doing}: {}", self.message),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An Artifacts-style remote is replaced whole wherever git quotes it (with or without a trailing slash), and its
    /// host alone where a network error names only the host, since both carry the account id.
    #[test]
    fn redaction_covers_the_url_and_its_host() {
        let remote = "https://0123abcd.artifacts.cloudflare.net/git/records/agent-records.git";
        let failure = Failure::failed(format!(
            "fatal: unable to access '{remote}/': Could not resolve host: 0123abcd.artifacts.cloudflare.net; \
             also {remote}"
        ))
        .redact_remote(remote);
        assert!(!failure.message.contains("0123abcd"), "{}", failure.message);
        assert_eq!(
            failure.message,
            "fatal: unable to access '<records remote>/': Could not resolve host: <records host>; also <records remote>"
        );
        assert_eq!(failure.code, 1);
    }

    /// A local-path remote (tests, or a bare repo on disk) has no host and is replaced as a whole string.
    #[test]
    fn local_path_remotes_are_replaced_whole() {
        let failure = Failure::failed("fatal: '/srv/r.git' does not appear to be a git repository")
            .redact_remote("/srv/r.git");
        assert_eq!(
            failure.message,
            "fatal: '<records remote>' does not appear to be a git repository"
        );
    }
}
