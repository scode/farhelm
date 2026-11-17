//! The JSON bodies of the helm's file-transfer routes: the hover check (`POST /api/sessions/{id}/files/stat`), the
//! native save into Downloads (`POST /api/sessions/{id}/files/download` with `save_to_downloads`), and the answer to an
//! attachment upload (`POST /api/sessions/{id}/attachments`).
//!
//! The helm encodes the answers and decodes the request with these types, and the GPUI app's helm client does the
//! opposite with the same ones. Until 2026-11-15 the helm kept them private and the client restated them, with only a
//! stub server pinning the answers; the migration's orchestrator asked that day for one definition both sides use, as
//! [`crate::session_requests`] is for launches.
//!
//! NOTE: This is a published wire format. Helms and desktop apps of different releases talk to each other through it,
//! so a field is never renamed, never changes between required and optional, and no new field is required on decode.
//! The tests below pin every type against literal JSON, byte for byte where the helm writes it.
//!
//! Not here: the upload's own request. Its bytes are the raw body and its proposed name is the `filename` query
//! parameter, which the client builds through [`crate::ui_channel::ApiPath`]'s percent-encoding rather than a serde
//! type, so there is no shared struct to hold it. The download's octet body is not JSON either.

use serde::{Deserialize, Serialize};

use crate::DownloadFileInfo;

/// The body of both `files/stat` and `files/download`: the path as the session printed it, which the session's host
/// resolves against the session's working directory and home.
///
/// `save_to_downloads` asks the desktop helm to save into Downloads and answer a [`DownloadReply`] instead of streaming
/// the bytes, and `open` additionally asks it to open the saved file; a helm without those capabilities refuses either.
/// Stat ignores both. The helm has always read a body that leaves either out as `false`, and a client of another
/// release may rely on that, so both stay optional on decode; the helm client always writes all three fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileRequest {
    pub path: String,
    #[serde(default)]
    pub save_to_downloads: bool,
    #[serde(default)]
    pub open: bool,
}

/// The helm's answer to a hover check: fresh evidence from the session's host, never cached.
///
/// `info` is the host's own report (resolved absolute path, size, and a typed refusal such as a folder or a file over
/// [`crate::DOWNLOAD_LIMIT_BYTES`]), flattened beside `host`, so a refusal is data in a successful answer rather than an
/// HTTP error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileStat {
    /// The host's display name, for the disclosure.
    pub host: String,
    /// Whether a modifier-click may open the saved file. Left out (`None`) by a helm with no native opener (a server
    /// helm), where opening is never offered; `Some(false)` when the file is not ready or its type is not on the list.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub can_open: Option<bool>,
    #[serde(flatten)]
    pub info: DownloadFileInfo,
}

/// The helm's answer to a native save: the saved file's local path, and what came of opening it when the request
/// asked to. `opened` is present only when the request asked to open, and `open_error` only when that open was refused
/// or failed; the save itself succeeded either way.
///
/// NOTE: The field order is the wire order, on purpose. Before the type was shared, the helm built this answer as a
/// `serde_json` object, adding `path`, then `opened`, then `open_error`. Only the desktop app's helm saves natively, and
/// in that binary the vendored GPUI crates turn on serde_json's `preserve_order`, so its objects kept that insertion
/// order. A struct serializes in field order whatever serde_json's features are, so this order now holds in every
/// build.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadReply {
    /// The saved file's path, under a no-clobber name (`report (1).pdf`) when the plain one was taken.
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opened: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_error: Option<String>,
}

/// The helm's answer to a published attachment upload: the attachment's absolute path on the session's host, as the
/// supervisor published it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UploadReply {
    pub path: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DownloadFileStatus;

    /// Decode `json` as `T`, check it is `expected`, and check `expected` encodes back to exactly `json`.
    fn pinned<T>(json: &str, expected: T)
    where
        T: Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
    {
        assert_eq!(serde_json::from_str::<T>(json).unwrap(), expected, "{json}");
        assert_eq!(serde_json::to_string(&expected).unwrap(), json);
    }

    /// Helms and desktop apps of different releases share these bodies, and both sides now derive them from one type,
    /// so a renamed field, a changed spelling or a reordered key would change both sides together and pass every test
    /// that only round-trips. Specified: each body's exact bytes, as the helm and the client wrote them before the
    /// types were shared: the request with all three fields in order, a stat answer from a server helm (no
    /// `can_open`, a null size, a snake_case refusal) and from the desktop helm, a save not asked to open (only
    /// `path`), one that opened, one whose open was refused (`path`, `opened`, `open_error`, as the desktop helm wrote
    /// them), and an upload's path. A stat answer without `can_open` decoding here is also what shows a missing
    /// `Option` needs no `#[serde(default)]`, flattened fields beside it included.
    #[farhelm_testtrace::test]
    fn every_body_decodes_from_and_encodes_to_its_literal_json() {
        let request = |save_to_downloads, open| FileRequest {
            path: "out/a.md".into(),
            save_to_downloads,
            open,
        };
        pinned(
            r#"{"path":"out/a.md","save_to_downloads":true,"open":true}"#,
            request(true, true),
        );
        let stat = |can_open, size, status| FileStat {
            host: "build-box".into(),
            can_open,
            info: DownloadFileInfo {
                path: "/home/u/a.md".into(),
                size,
                status,
            },
        };
        pinned(
            r#"{"host":"build-box","path":"/home/u/a.md","size":null,"status":"not_readable"}"#,
            stat(None, None, DownloadFileStatus::NotReadable),
        );
        pinned(
            r#"{"host":"build-box","can_open":false,"path":"/home/u/a.md","size":100000001,"status":"too_large"}"#,
            stat(Some(false), Some(100_000_001), DownloadFileStatus::TooLarge),
        );
        let saved = |opened, open_error: Option<&str>| DownloadReply {
            path: "/dl/a (1).md".into(),
            opened,
            open_error: open_error.map(Into::into),
        };
        pinned(r#"{"path":"/dl/a (1).md"}"#, saved(None, None));
        pinned(
            r#"{"path":"/dl/a (1).md","opened":true}"#,
            saved(Some(true), None),
        );
        pinned(
            r#"{"path":"/dl/a (1).md","opened":false,"open_error":"opening downloaded files is turned off in settings"}"#,
            saved(
                Some(false),
                Some("opening downloaded files is turned off in settings"),
            ),
        );
        pinned(
            r#"{"path":"/att/s1/shot.png"}"#,
            UploadReply {
                path: "/att/s1/shot.png".into(),
            },
        );
    }

    /// The helm has always accepted a request body that leaves `open`, or both flags, out, and a client of another
    /// release may send one, so neither flag may become required. Specified: missing flags decode as `false`. (A save
    /// answer without the open fields is pinned above.)
    #[farhelm_testtrace::test]
    fn optional_fields_may_be_left_out() {
        let request: FileRequest = serde_json::from_str(r#"{"path":"a.md"}"#).unwrap();
        assert_eq!((request.save_to_downloads, request.open), (false, false));
        let request: FileRequest =
            serde_json::from_str(r#"{"path":"a.md","save_to_downloads":true}"#).unwrap();
        assert_eq!((request.save_to_downloads, request.open), (true, false));
    }
}
