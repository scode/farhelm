//! A UI-channel command's path: the one type a command can carry as its target.
//!
//! Paths are a trust boundary (SPEC_impl.md, "Paths are a trust boundary"). Ids that come from a supervisor, such as
//! a session's tab ids, get no check on the helm side, so a host that reports a tab id like `../../victim` must not be
//! able to make the GUI's "close this tab" retarget another resource. Two things keep that from happening, and both
//! live in this one type, so the GUI's builder and the helm's check cannot drift apart:
//!
//! - **Building** ([`ApiPath::api`], [`ApiPath::with_query`]) percent-encodes every segment on its own, and refuses a
//!   segment that is empty, `.` or `..`, because no encoding of those is a valid resource name. Feature code cannot
//!   format a path by hand: the field a command carries is this type, there is no public constructor from a string,
//!   and every path the builder can produce decodes again unchanged.
//! - **Decoding** (deserialization, the only way to turn a string into this type) accepts a path only if the builder
//!   could have produced it: `/api/` followed by non-empty segments, none of them `.` or `..` (also percent-encoded), each
//!   segment and query component spelled exactly as the builder encodes it (so no raw separators, spaces, fragments,
//!   lowercase or needless escapes); or exactly the channel-local [`SESSION_QUERY_PATH`]. Anything a URL normalizer
//!   would rewrite is refused rather than normalized. On the helm, a command whose path does not parse does not decode
//!   at all, so it never reaches the router.
//!
//! A segment may still carry an encoded `/` or `\` (a host's id can contain one, and terminal stream paths address
//! such ids). The helm's command dispatcher refuses those with an ordinary reply ([`ApiPath::encodes_a_separator`]),
//! because routes decode a path parameter before using it; that is a policy about what a command may address, not a
//! question of canonical spelling, so a GUI that meets such an id gets a refusal for that one action, never a reset.

use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::SESSION_QUERY_PATH;

/// What stays unencoded in a path segment or query component: RFC 3986's unreserved characters. Everything else,
/// `/`, `?`, `%`, `#` and `\` included, is percent-encoded, so a segment can never split into two or carry a query.
const COMPONENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

/// A canonical command path: `/api/` followed by percent-encoded segments, optionally with a query; or the
/// channel-local session-query path. See the module docs for what canonical means and why it matters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiPath(String);

/// Why a segment or a path is not canonical.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PathError {
    #[error("a path segment is empty")]
    EmptySegment,
    #[error("a command path needs at least one segment after /api")]
    NoSegments,
    #[error("a path segment is `.` or `..`")]
    DotSegment,
    #[error("a command path must start with /api/")]
    NotApi,
    #[error("a command path is not spelled the way the path builder encodes it")]
    NotCanonical,
    #[error("the channel-local session-query path takes no query")]
    QueryOnChannelPath,
}

impl ApiPath {
    /// `/api/<segments...>`, each segment encoded on its own. Refuses an empty list (bare `/api` addresses nothing and
    /// would not decode) and empty, `.` and `..` segments.
    pub fn api(segments: &[&str]) -> Result<ApiPath, PathError> {
        if segments.is_empty() {
            return Err(PathError::NoSegments);
        }
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

    /// The same path with a query of `key=value` pairs appended, each key and value encoded on its own. Refused on the
    /// channel-local session-query path, which the helm handles itself and which decodes only without a query.
    pub fn with_query(mut self, pairs: &[(&str, &str)]) -> Result<ApiPath, PathError> {
        if self.0 == SESSION_QUERY_PATH {
            return Err(PathError::QueryOnChannelPath);
        }
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
        Ok(self)
    }

    /// The channel-local command that sets this channel's session-list query ([`SESSION_QUERY_PATH`]); the helm
    /// handles it itself and never routes it.
    pub fn session_query() -> ApiPath {
        ApiPath(SESSION_QUERY_PATH.to_string())
    }

    /// Accept `path` only if the builder could have produced it (module docs). This is what a received command's path
    /// goes through, so the helm refuses here what a correct GUI can never send. Private on purpose: a public string
    /// constructor would let GUI code format a path by hand (`format!(".../tabs/{tab}")`) and get it accepted, and
    /// a host-supplied `victim/close` would then retarget the request. Only deserialization calls it.
    fn parse(path: &str) -> Result<ApiPath, PathError> {
        if path == SESSION_QUERY_PATH {
            return Ok(ApiPath::session_query());
        }
        let (route, query) = match path.split_once('?') {
            Some((route, query)) => (route, Some(query)),
            None => (path, None),
        };
        let Some(rest) = route.strip_prefix("/api/") else {
            return Err(PathError::NotApi);
        };
        for segment in rest.split('/') {
            if segment.is_empty() {
                return Err(PathError::EmptySegment);
            }
            let decoded = decode_component(segment)?;
            if decoded == "." || decoded == ".." {
                return Err(PathError::DotSegment);
            }
        }
        if let Some(query) = query {
            for pair in query.split('&') {
                let (key, value) = pair.split_once('=').ok_or(PathError::NotCanonical)?;
                decode_component(key)?;
                decode_component(value)?;
            }
        }
        Ok(ApiPath(path.to_string()))
    }

    /// Whether a path segment decodes to text holding a `/` or `\`. Such a path is canonical (the builder encodes a
    /// host id that contains one), but the helm refuses it as a command: routes decode a path parameter before using
    /// it, and an id that turns into two segments there must not address anything.
    pub fn encodes_a_separator(&self) -> bool {
        let route = self
            .0
            .split_once('?')
            .map_or(self.0.as_str(), |(route, _)| route);
        route.split('/').any(|segment| {
            percent_decode_str(segment)
                .decode_utf8_lossy()
                .contains(['/', '\\'])
        })
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Decode one segment or query component, refusing it unless it is spelled exactly as [`COMPONENT`] encodes its
/// decoded text: that rules out raw separators and spaces, malformed or lowercase escapes, and escapes of characters
/// that need none, so every accepted path has one spelling.
fn decode_component(component: &str) -> Result<String, PathError> {
    let decoded = percent_decode_str(component)
        .decode_utf8()
        .map_err(|_| PathError::NotCanonical)?;
    if utf8_percent_encode(&decoded, COMPONENT).to_string() != component {
        return Err(PathError::NotCanonical);
    }
    Ok(decoded.into_owned())
}

impl Serialize for ApiPath {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ApiPath {
    /// Through the private parse: a non-canonical path fails the whole message's decode.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let path = String::deserialize(deserializer)?;
        ApiPath::parse(&path).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A host-supplied id is data, never path structure: the helm does not check tab ids, so the builder is what keeps
    /// `../../victim` from addressing another resource. Refusing dot segments outright (rather than encoding them)
    /// matches the parse, which refuses `%2E%2E` too.
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
            .with_query(&[("only_if", "a&b=c"), ("x", "1")])
            .unwrap();
        assert_eq!(path.as_str(), "/api/sessions/s1?only_if=a%26b%3Dc&x=1");
    }

    /// The parse is the helm's whole defence against a path that retargets a request, so each form a URL normalizer
    /// would rewrite, and anything else the builder would have spelled differently, is refused; ordinary encoded ids
    /// (including ones that look like separators once decoded only partly) pass, so refusing is not over-broad.
    #[test]
    fn non_canonical_paths_are_refused_and_canonical_ones_pass() {
        for refused in [
            "/api/sessions/../hosts",
            "/api/sessions/./x",
            "/api/sessions/%2e%2e/hosts",
            "/api/sessions/%2E/x",
            "/api/sessions//x",
            "/api/sessions/x/",
            "/api/sessions/a\\b",
            "/api/sessions/x#frag",
            "/api/x?q=a#b",
            "/ui/elsewhere",
            "api/sessions",
            "/api/sessions/a b",
            "/api/sessions/x?q=a b",
            "/api/x\t",
            "/api/sessions/%zz",
            "/api/sessions/%4",
            "/api/sessions/\u{e9}",
            // Spelled differently from the builder: a lowercase escape, an escape of a character that needs none,
            // a raw separator in a query value, a query pair without `=`.
            "/api/sessions/a%3ab",
            "/api/sessions/%61",
            "/api/browse-directory?path=/a/../b",
            "/api/sessions?flag",
        ] {
            assert!(ApiPath::parse(refused).is_err(), "should refuse {refused}");
        }
        for allowed in [
            "/api/sessions",
            "/api/sessions/00000000-0000-4000-8000-000000000001%3As1/rename",
            "/api/sessions/x?only_if_nothing_alive=true",
            "/api/browse-directory?path=%2Fa%2F..%2Fb",
            "/api/sessions/..x",
            "/api/sessions/a%2Fb",
            SESSION_QUERY_PATH,
        ] {
            assert!(ApiPath::parse(allowed).is_ok(), "should allow {allowed}");
        }
    }

    /// An id holding a separator is canonical (the builder encodes it, and terminal stream paths address such ids),
    /// but the helm must still be able to tell, because routes decode it into two segments. Only path segments count:
    /// a query value holding `/` is just data.
    #[test]
    fn an_encoded_separator_in_a_segment_is_detected() {
        let id = |id: &str| ApiPath::api(&["sessions", id, "rename"]).unwrap();
        assert!(id("a/b").encodes_a_separator());
        assert!(id("a\\b").encodes_a_separator());
        assert!(!id("a:b").encodes_a_separator());
        let query = ApiPath::api(&["browse-directory"])
            .unwrap()
            .with_query(&[("path", "/a/b")])
            .unwrap();
        assert!(!query.encodes_a_separator());
    }

    /// Whatever the builder produces, the parse accepts unchanged: the GUI never builds a path the helm refuses, since
    /// a path that fails to decode resets the whole channel. The two shapes that would not decode (bare `/api`, and a
    /// query on the session-query path) are refused when built instead.
    #[test]
    fn every_built_path_parses_back_to_itself() {
        let built = [
            ApiPath::api(&["sessions", "a?b#c\\d e%", "tabs", "../../victim"])
                .unwrap()
                .with_query(&[("path", "/a/../b"), ("q", "x y&z")])
                .unwrap(),
            ApiPath::api(&["sessions", "\u{e9}t\u{e9}"]).unwrap(),
            ApiPath::api(&["hosts"]).unwrap().with_query(&[]).unwrap(),
            ApiPath::session_query(),
        ];
        for path in built {
            assert_eq!(ApiPath::parse(path.as_str()), Ok(path));
        }
        assert_eq!(ApiPath::api(&[]), Err(PathError::NoSegments));
        assert_eq!(
            ApiPath::session_query().with_query(&[("x", "1")]),
            Err(PathError::QueryOnChannelPath)
        );
    }

    /// The wire carries a path as a plain string, and decoding one goes through the parse, so a message carrying a
    /// non-canonical path does not decode at all.
    #[test]
    fn paths_serialize_as_strings_and_decode_only_when_canonical() {
        let path = ApiPath::api(&["hosts"]).unwrap();
        assert_eq!(serde_json::to_string(&path).unwrap(), "\"/api/hosts\"");
        assert_eq!(
            serde_json::from_str::<ApiPath>("\"/api/hosts\"").unwrap(),
            path
        );
        assert!(serde_json::from_str::<ApiPath>("\"/api/sessions/%2e%2e/hosts\"").is_err());
    }
}
