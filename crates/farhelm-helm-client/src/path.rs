//! The one way to build a command's path.
//!
//! Paths are a trust boundary (SPEC_impl.md, "Paths are a trust boundary"). Ids that come from a supervisor, such as
//! a session's tab ids, get no check on the helm side, so a host that reports a tab id like `../../victim` must not be
//! able to make the GUI's "close this tab" retarget another resource. The helm refuses non-canonical paths, and this
//! builder makes sure the GUI never sends one: every segment is percent-encoded on its own, and a segment that is
//! empty, `.` or `..` is refused here rather than encoded, because no encoding of it is a valid resource name.
//!
//! [`ApiPath`] can only be made through this module, so a command path cannot be a string feature code formatted by
//! hand.

use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};

/// What stays unencoded in a path segment or query component: RFC 3986's unreserved characters. Everything else,
/// `/`, `?`, `%`, `#` and `\` included, is percent-encoded, so a segment can never split into two or carry a query.
const COMPONENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

/// A command path the helm will accept as canonical: `/api/` followed by percent-encoded segments, optionally a
/// percent-encoded query; or the channel-local session-query path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiPath(String);

/// A segment that has no canonical encoding as a resource name.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PathError {
    #[error("a path segment is empty")]
    EmptySegment,
    #[error("a path segment is `.` or `..`")]
    DotSegment,
}

impl ApiPath {
    /// `/api/<segments...>`, each segment encoded on its own. Refuses empty, `.` and `..` segments.
    pub fn api(segments: &[&str]) -> Result<ApiPath, PathError> {
        let mut path = String::from("/api");
        for segment in segments {
            match *segment {
                "" => return Err(PathError::EmptySegment),
                "." | ".." => return Err(PathError::DotSegment),
                _ => {}
            }
            path.push('/');
            path.extend(utf8_percent_encode(segment, COMPONENT));
        }
        Ok(ApiPath(path))
    }

    /// The same path with a query of `key=value` pairs appended, each key and value encoded on its own.
    pub fn with_query(mut self, pairs: &[(&str, &str)]) -> ApiPath {
        for (index, (key, value)) in pairs.iter().enumerate() {
            self.0.push(if index == 0 && !self.0.contains('?') {
                '?'
            } else {
                '&'
            });
            self.0.extend(utf8_percent_encode(key, COMPONENT));
            self.0.push('=');
            self.0.extend(utf8_percent_encode(value, COMPONENT));
        }
        self
    }

    /// The channel-local command that sets this channel's session-list query
    /// ([`farhelm_proto::ui_channel::SESSION_QUERY_PATH`]); the helm handles it itself and never routes it.
    pub fn session_query() -> ApiPath {
        ApiPath(farhelm_proto::ui_channel::SESSION_QUERY_PATH.to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A host-supplied id is data, never path structure: the helm does not check tab ids, so the builder is what keeps
    /// `../../victim` from addressing another resource. Refusing dot segments outright (rather than encoding them)
    /// matches the helm, which refuses `%2E%2E` too.
    #[test]
    fn host_supplied_ids_cannot_change_the_path_structure() {
        let path = ApiPath::api(&["sessions", "s1", "tabs", "../../victim"]).unwrap();
        assert_eq!(path.as_str(), "/api/sessions/s1/tabs/..%2F..%2Fvictim");
        assert_eq!(
            ApiPath::api(&["sessions", ".."]),
            Err(PathError::DotSegment)
        );
        assert_eq!(ApiPath::api(&["sessions", "."]), Err(PathError::DotSegment));
        assert_eq!(
            ApiPath::api(&["sessions", ""]),
            Err(PathError::EmptySegment)
        );
        let sneaky = ApiPath::api(&["sessions", "a?b#c\\d e%"]).unwrap();
        assert_eq!(sneaky.as_str(), "/api/sessions/a%3Fb%23c%5Cd%20e%25");
    }

    /// Query values are encoded too, so a value cannot add a second parameter.
    #[test]
    fn query_values_are_encoded_on_their_own() {
        let path = ApiPath::api(&["sessions", "s1"])
            .unwrap()
            .with_query(&[("only_if", "a&b=c"), ("x", "1")]);
        assert_eq!(path.as_str(), "/api/sessions/s1?only_if=a%26b%3Dc&x=1");
    }
}
