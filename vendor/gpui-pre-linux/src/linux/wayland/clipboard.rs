// Modified for Farhelm: changed or added by the patch layers in vendor/patches/gpui/ (vendor/README.md, "GPUI").
use std::{
    fs::File,
    io::{ErrorKind, Write},
    os::fd::{AsRawFd, BorrowedFd, OwnedFd},
};

use calloop::{LoopHandle, PostAction};
use filedescriptor::Pipe;
use std::{cell::Cell, path::PathBuf, rc::Rc};
use strum::IntoEnumIterator;
use wayland_client::{Connection, Proxy, protocol::wl_data_offer::WlDataOffer};
use wayland_protocols::wp::primary_selection::zv1::client::zwp_primary_selection_offer_v1::ZwpPrimarySelectionOfferV1;

use crate::linux::platform::{PIPE_READ_TIMEOUT, read_fd_with_timeout};
use gpui::{
    BackgroundExecutor, ClipboardEntry, ClipboardItem, ClipboardReadError, ExternalPaths,
    ForegroundExecutor, Image, ImageFormat, Task, hash,
};

/// Text mime types that we'll offer to other programs.
pub(crate) const TEXT_MIME_TYPES: [&str; 3] =
    ["text/plain;charset=utf-8", "UTF8_STRING", "text/plain"];
pub(crate) const FILE_LIST_MIME_TYPE: &str = "text/uri-list";

/// Text mime types that we'll accept from other programs.
pub(crate) const ALLOWED_TEXT_MIME_TYPES: [&str; 2] = ["text/plain;charset=utf-8", "UTF8_STRING"];

/// Which flavors an async read asks another program's selection for, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReadOrder {
    /// A text field's paste (`read_from_clipboard_async`): text, then an image, as the synchronous read used to ask.
    Text,
    /// A paste that can take files or an image (`read_from_clipboard_payload_async`): local files, then an image, then
    /// text, so a file manager's copy attaches its files rather than typing their paths.
    Payload,
}

pub(crate) struct Clipboard {
    connection: Connection,
    loop_handle: LoopHandle<'static, ()>,
    self_mime: String,

    // Internal clipboard
    contents: Option<ClipboardItem>,
    primary_contents: Option<ClipboardItem>,

    // External clipboard
    current_offer: Option<DataOffer<WlDataOffer>>,
    /// Selection events can supersede a proxy that is still alive in the compositor.
    offer_generation: Rc<Cell<u64>>,
    cached_primary_read: Option<ClipboardItem>,
    current_primary_offer: Option<DataOffer<ZwpPrimarySelectionOfferV1>>,
}

#[cfg(test)]
mod attachment_clipboard_tests {
    use super::*;

    /// The policy test must choose a flavor without touching native transport.
    struct NoReceive;

    impl ReceiveData for NoReceive {
        fn receive_data(&self, _: String, _: BorrowedFd<'_>) {
            panic!("MIME selection must not perform clipboard IO");
        }
    }

    /// Mixed offers must request the image's actual offered MIME (including
    /// legacy aliases), independent of producer ordering. This pins async flavor
    /// policy only; it does not claim a native compositor or pipe-read check.
    #[test]
    fn attachment_clipboard_image_priority_and_aliases() {
        for (mimes, expected) in [
            (
                vec!["text/plain", "image/png"],
                Some(("image/png", Some(ImageFormat::Png))),
            ),
            (
                vec!["image/png", "text/plain"],
                Some(("image/png", Some(ImageFormat::Png))),
            ),
            (
                vec!["text/plain;charset=utf-8", "image/jpg"],
                Some(("image/jpg", Some(ImageFormat::Jpeg))),
            ),
            (
                vec!["text/plain", "image/tif"],
                Some(("image/tif", Some(ImageFormat::Tiff))),
            ),
            (vec!["text/plain"], Some(("text/plain", None))),
            (
                vec!["text/plain", "text/plain;charset=utf-8"],
                Some(("text/plain;charset=utf-8", None)),
            ),
            (vec!["application/octet-stream"], None),
        ] {
            let mut offer = DataOffer::new(NoReceive);
            for mime in mimes {
                offer.add_mime_type(mime.into());
            }
            assert_eq!(offer.payload_mime_type(), expected);
        }
    }

    /// An offer advertises flavors before any payload arrives. The test reader can then supply exactly the bytes
    /// a producer would write, and record which flavors the paste actually requested.
    fn offer_with(mimes: &[&str]) -> DataOffer<NoReceive> {
        let mut offer = DataOffer::new(NoReceive);
        for mime in mimes {
            offer.add_mime_type((*mime).into());
        }
        offer
    }

    /// A file manager's mixed offer can contain comments, blank lines, and non-file URIs. Only local files become
    /// attachments, in their original order, and the presence of a local file ends MIME selection.
    #[test]
    fn attachment_clipboard_prefers_local_files_from_a_mixed_list() {
        let offer = offer_with(&["text/plain", "image/png", FILE_LIST_MIME_TYPE]);
        let item = smol::block_on(offer.read_payload(
            ReadOrder::Payload,
            || true,
            |mime, format| {
                assert_eq!(mime, FILE_LIST_MIME_TYPE);
                assert_eq!(format, None);
                Task::ready(decode_payload(
                    b"# files\r\nfile:///files/a%20b\r\n\r\nhttps://example.org/a\n\xff\n\
                      file://server/files/c\nfile://server/c:/remote\nfile:///files/invalid%q0\n\
                      file://localhost/files/d\nfile:/files/e\nfile:relative\n"
                        .to_vec(),
                    true,
                    format,
                ))
            },
        ))
        .unwrap()
        .unwrap();
        let [ClipboardEntry::ExternalPaths(paths)] = item.entries.as_slice() else {
            panic!("a valid file list supplies a file payload");
        };
        assert_eq!(
            paths.0.as_slice(),
            [
                PathBuf::from("/files/a b"),
                PathBuf::from("/files/d"),
                PathBuf::from("/files/e"),
            ]
        );
    }

    /// Copy Link may advertise a URI list and text together. The text must still paste, and an offered image
    /// keeps its precedence over text when the URI list yields no local files.
    #[test]
    fn attachment_clipboard_falls_back_on_the_same_offer() {
        let link = "https://localhost/guide";
        for (mimes, fallback) in [
            (vec![FILE_LIST_MIME_TYPE, "text/plain"], "text/plain"),
            (
                vec!["image/png", FILE_LIST_MIME_TYPE, "text/plain"],
                "image/png",
            ),
        ] {
            let offer = offer_with(&mimes);
            let mut requested = Vec::new();
            let item = smol::block_on(offer.read_payload(
                ReadOrder::Payload,
                || true,
                |mime, format| {
                    requested.push(mime.to_owned());
                    let bytes = if mime == "image/png" {
                        b"image".as_slice()
                    } else {
                        link.as_bytes()
                    };
                    Task::ready(decode_payload(
                        bytes.to_vec(),
                        mime == FILE_LIST_MIME_TYPE,
                        format,
                    ))
                },
            ))
            .unwrap()
            .unwrap();
            assert_eq!(requested, [FILE_LIST_MIME_TYPE, fallback]);
            if fallback == "image/png" {
                let [ClipboardEntry::Image(image)] = item.entries.as_slice() else {
                    panic!("the image outranks text");
                };
                assert_eq!(image.format, ImageFormat::Png);
                assert_eq!(image.bytes.as_slice(), b"image");
            } else {
                assert_eq!(item.text().as_deref(), Some(link));
            }
        }
    }

    /// A selection change while the file pipe is open ends that paste. The old reply supplies no text from either
    /// the old or new selection, even though the original offer had plain text available.
    #[test]
    fn attachment_clipboard_cancels_if_the_offer_expires_mid_read() {
        let offer = offer_with(&[FILE_LIST_MIME_TYPE, "text/plain"]);
        let live = Cell::new(true);
        let mut requested = Vec::new();
        let result = smol::block_on(offer.read_payload(
            ReadOrder::Payload,
            || live.get(),
            |mime, _| {
                requested.push(mime.to_owned());
                live.set(false);
                Task::ready(Ok(None))
            },
        ));
        assert!(result.unwrap().is_none());
        assert_eq!(requested, [FILE_LIST_MIME_TYPE]);

        let result = smol::block_on(offer.read_payload(
            ReadOrder::Payload,
            || live.get(),
            |_, _| panic!("an expired offer cannot receive"),
        ));
        assert!(result.unwrap().is_none());
    }

    /// A stalled or failed pipe is a transfer failure. Only a successfully read empty list permits a new request.
    #[test]
    fn attachment_clipboard_does_not_fall_back_after_a_pipe_error() {
        let offer = offer_with(&[FILE_LIST_MIME_TYPE, "text/plain"]);
        let result = smol::block_on(offer.read_payload(
            ReadOrder::Payload,
            || true,
            |mime, _| {
                assert_eq!(mime, FILE_LIST_MIME_TYPE);
                Task::ready(Err(ClipboardReadError::UnsupportedContent))
            },
        ));
        assert!(matches!(
            result,
            Err(ClipboardReadError::UnsupportedContent)
        ));
    }

    /// Why this matters: a text field pastes only text, and its paste on Wayland now always takes the async read.
    /// If that read kept the terminal's order, copying a file in a file manager or an image with a caption in a
    /// browser would paste nothing into a field, though the clipboard holds text, where the old synchronous read
    /// pasted the text.
    ///
    /// Specification: the text read asks a mixed offer for its text flavor first and only for that (whatever order
    /// the producer listed its flavors in, and with a file list, an image or both beside it), and hands back the
    /// text with CRLF made LF; an offer with no text flavor is asked for its image; one with only a file list is
    /// asked for nothing.
    #[test]
    fn attachment_clipboard_text_read_takes_text_from_a_mixed_offer() {
        for mimes in [
            vec!["image/png", "text/plain;charset=utf-8"],
            vec!["text/plain;charset=utf-8", "image/png"],
            vec![FILE_LIST_MIME_TYPE, "text/plain;charset=utf-8"],
            vec![
                FILE_LIST_MIME_TYPE,
                "image/png",
                "UTF8_STRING",
                "text/plain",
            ],
        ] {
            let offer = offer_with(&mimes);
            let mut requested = Vec::new();
            let item = smol::block_on(offer.read_payload(
                ReadOrder::Text,
                || true,
                |mime, format| {
                    requested.push(mime.to_owned());
                    Task::ready(decode_payload(b"fix\r\nbug".to_vec(), false, format))
                },
            ))
            .unwrap()
            .unwrap();
            let text = if mimes.contains(&"UTF8_STRING") {
                "UTF8_STRING"
            } else {
                "text/plain;charset=utf-8"
            };
            assert_eq!(requested, [text], "{mimes:?}: only the text flavor");
            assert_eq!(item.text().as_deref(), Some("fix\nbug"), "{mimes:?}");
        }

        let offer = offer_with(&[FILE_LIST_MIME_TYPE, "image/png"]);
        let mut requested = Vec::new();
        let item = smol::block_on(offer.read_payload(
            ReadOrder::Text,
            || true,
            |mime, format| {
                requested.push(mime.to_owned());
                Task::ready(decode_payload(b"image".to_vec(), false, format))
            },
        ))
        .unwrap()
        .unwrap();
        assert_eq!(
            requested,
            ["image/png"],
            "no text: the image, never the list"
        );
        assert!(matches!(
            item.entries.as_slice(),
            [ClipboardEntry::Image(_)]
        ));

        let offer = offer_with(&[FILE_LIST_MIME_TYPE]);
        let result = smol::block_on(offer.read_payload(
            ReadOrder::Text,
            || true,
            |_, _| panic!("a file list alone has nothing for a text field"),
        ));
        assert!(result.unwrap().is_none());
    }

    /// Why this matters: the terminal's paste keeps its own order on the new payload read. Pinned beside the text
    /// read's test so that the two orders cannot quietly become one.
    ///
    /// Specification: on an offer listing text, an image and a file list, the payload read asks for the file list
    /// first, and after an empty list for the image, never the text; its text, when text is all there is, keeps its
    /// CRLF line endings, which the terminal's paste leaves to the supervisor.
    #[test]
    fn attachment_clipboard_payload_read_keeps_files_then_image_then_text() {
        let offer = offer_with(&["text/plain;charset=utf-8", "image/png", FILE_LIST_MIME_TYPE]);
        let mut requested = Vec::new();
        smol::block_on(offer.read_payload(
            ReadOrder::Payload,
            || true,
            |mime, format| {
                requested.push(mime.to_owned());
                Task::ready(decode_payload(
                    b"https://example.org/\n".to_vec(),
                    mime == FILE_LIST_MIME_TYPE,
                    format,
                ))
            },
        ))
        .unwrap()
        .unwrap();
        assert_eq!(requested, [FILE_LIST_MIME_TYPE, "image/png"]);

        let offer = offer_with(&["text/plain;charset=utf-8"]);
        let item = smol::block_on(offer.read_payload(
            ReadOrder::Payload,
            || true,
            |_, format| Task::ready(decode_payload(b"a\r\nb".to_vec(), false, format)),
        ))
        .unwrap()
        .unwrap();
        assert_eq!(item.text().as_deref(), Some("a\r\nb"));
    }
}

pub(crate) trait ReceiveData {
    fn receive_data(&self, mime_type: String, fd: BorrowedFd<'_>);
}

impl ReceiveData for WlDataOffer {
    fn receive_data(&self, mime_type: String, fd: BorrowedFd<'_>) {
        self.receive(mime_type, fd);
    }
}

impl ReceiveData for ZwpPrimarySelectionOfferV1 {
    fn receive_data(&self, mime_type: String, fd: BorrowedFd<'_>) {
        self.receive(mime_type, fd);
    }
}

#[derive(Clone, Debug)]
/// Wrapper for `WlDataOffer` and `ZwpPrimarySelectionOfferV1`, used to help track mime types.
pub(crate) struct DataOffer<T: ReceiveData> {
    pub inner: T,
    mime_types: Vec<String>,
}

impl<T: ReceiveData> DataOffer<T> {
    pub fn new(offer: T) -> Self {
        Self {
            inner: offer,
            mime_types: Vec::new(),
        }
    }

    pub fn add_mime_type(&mut self, mime_type: String) {
        self.mime_types.push(mime_type)
    }

    fn has_mime_type(&self, mime_type: &str) -> bool {
        self.mime_types.iter().any(|t| t == mime_type)
    }

    /// The offered image flavor, as the exact offered MIME (including legacy aliases) and its format.
    fn image_mime_type(&self) -> Option<(&str, Option<ImageFormat>)> {
        ImageFormat::iter().find_map(|format| {
            self.mime_types
                .iter()
                .find(|mime| ImageFormat::from_mime_type(mime) == Some(format))
                .map(|mime| (mime.as_str(), Some(format)))
        })
    }

    /// The offered text flavor, in [`TEXT_MIME_TYPES`]' order of preference.
    fn text_mime_type(&self) -> Option<(&str, Option<ImageFormat>)> {
        TEXT_MIME_TYPES.iter().find_map(|preferred| {
            self.mime_types
                .iter()
                .find(|mime| mime.as_str() == *preferred)
                .map(|mime| (mime.as_str(), None))
        })
    }

    /// The payload read's flavor after files: an image over text, requested as the exact offered MIME.
    fn payload_mime_type(&self) -> Option<(&str, Option<ImageFormat>)> {
        self.image_mime_type().or_else(|| self.text_mime_type())
    }

    /// The flavors `order` asks this offer for, in turn. Only an empty file list moves on to the next one.
    fn flavors(&self, order: ReadOrder) -> Vec<(&str, Option<ImageFormat>)> {
        match order {
            ReadOrder::Text => self
                .text_mime_type()
                .into_iter()
                .chain(self.image_mime_type())
                .collect(),
            ReadOrder::Payload => self
                .has_mime_type(FILE_LIST_MIME_TYPE)
                .then_some((FILE_LIST_MIME_TYPE, None))
                .into_iter()
                .chain(self.payload_mime_type())
                .collect(),
        }
    }

    /// An empty file list permits fallback on this offer. Losing the offer or failing to read its pipe ends the
    /// request. The caller supplies the current-selection check independently of the lifetime of the Wayland proxy.
    async fn read_payload(
        &self,
        order: ReadOrder,
        is_current: impl Fn() -> bool,
        mut read: impl FnMut(
            &str,
            Option<ImageFormat>,
        ) -> Task<Result<Option<ClipboardItem>, ClipboardReadError>>,
    ) -> Result<Option<ClipboardItem>, ClipboardReadError> {
        for (mime, format) in self.flavors(order) {
            if !is_current() {
                return Ok(None);
            }
            let result = read(mime, format).await;
            if !is_current() {
                return Ok(None);
            }
            if let Some(item) = result? {
                // A text field gets the line endings the synchronous read gave it: Firefox, for one, copies CRLF,
                // which a field would otherwise show as blank lines.
                return Ok(Some(match (order, format) {
                    (ReadOrder::Text, None) => unix_line_endings(item),
                    _ => item,
                }));
            }
        }
        Ok(None)
    }

    /// Request one flavor on the Wayland thread. Only the pipe and its representation cross to the background worker.
    fn receive_payload(
        &self,
        connection: &Connection,
        executor: &BackgroundExecutor,
        mime: &str,
        format: Option<ImageFormat>,
    ) -> Task<Result<Option<ClipboardItem>, ClipboardReadError>> {
        let pipe = match Pipe::new() {
            Ok(pipe) => pipe,
            Err(error) => {
                log::error!("creating async clipboard pipe: {error}");
                return Task::ready(Err(ClipboardReadError::Unavailable));
            }
        };
        self.inner.receive_data(mime.to_owned(), unsafe {
            BorrowedFd::borrow_raw(pipe.write.as_raw_fd())
        });
        let fd = pipe.read;
        drop(pipe.write);
        if let Err(error) = connection.flush() {
            log::error!("flushing async clipboard receive: {error}");
            return Task::ready(Err(ClipboardReadError::Unavailable));
        }
        let file_list = mime == FILE_LIST_MIME_TYPE;
        executor.spawn(async move {
            // Dropping the paste can cancel later work, but a pipe read already running keeps its per-poll timeout.
            let bytes = read_fd_with_timeout(fd, PIPE_READ_TIMEOUT).map_err(|error| {
                log::error!("reading async clipboard pipe: {error}");
                ClipboardReadError::UnsupportedContent
            })?;
            decode_payload(bytes, file_list, format)
        })
    }

    fn read_bytes(&self, connection: &Connection, mime_type: &str) -> Option<Vec<u8>> {
        let pipe = Pipe::new().unwrap();
        self.inner.receive_data(mime_type.to_string(), unsafe {
            BorrowedFd::borrow_raw(pipe.write.as_raw_fd())
        });
        let fd = pipe.read;
        drop(pipe.write);

        connection.flush().unwrap();

        match read_fd_with_timeout(fd, PIPE_READ_TIMEOUT) {
            Ok(bytes) => Some(bytes),
            Err(err) => {
                log::error!("error reading clipboard pipe: {err:?}");
                None
            }
        }
    }

    fn read_text(&self, connection: &Connection) -> Option<ClipboardItem> {
        let mime_type = self.mime_types.iter().find(|&mime_type| {
            ALLOWED_TEXT_MIME_TYPES
                .iter()
                .any(|&allowed| allowed == mime_type)
        })?;
        let bytes = self.read_bytes(connection, mime_type)?;
        let text_content = match String::from_utf8(bytes) {
            Ok(content) => content,
            Err(e) => {
                log::error!("Failed to convert clipboard content to UTF-8: {}", e);
                return None;
            }
        };

        // Normalize the text to unix line endings, otherwise
        // copying from eg: firefox inserts a lot of blank
        // lines, and that is super annoying.
        let result = text_content.replace("\r\n", "\n");
        Some(ClipboardItem::new_string(result))
    }

    fn read_image(&self, connection: &Connection) -> Option<ClipboardItem> {
        for format in ImageFormat::iter() {
            let mime_type = format.mime_type();
            if !self.has_mime_type(mime_type) {
                continue;
            }

            if let Some(bytes) = self.read_bytes(connection, mime_type) {
                let id = hash(&bytes);
                return Some(ClipboardItem {
                    entries: vec![ClipboardEntry::Image(Image { format, bytes, id })],
                });
            }
        }
        None
    }
}

/// URI list comments, malformed lines and non-local URLs do not contribute file paths. Decode each line separately
/// so a damaged entry cannot hide neighboring local files.
fn parse_file_uri_list(bytes: &[u8]) -> ExternalPaths {
    ExternalPaths(
        bytes
            .split(|byte| *byte == b'\n')
            .filter_map(|line| std::str::from_utf8(line).ok())
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .filter_map(local_file_uri)
            .collect(),
    )
}

/// The URL parser can repair malformed input, including dropping a remote authority before a drive-letter path.
/// Use the parser's syntax notices to keep those repaired URLs out of the local file list.
fn local_file_uri(line: &str) -> Option<PathBuf> {
    if !line.get(..6)?.eq_ignore_ascii_case("file:/") {
        return None;
    }
    let invalid = Cell::new(false);
    let note_violation = |violation| {
        // A file URI can use a single slash when it gives an absolute path without an authority.
        if violation != url::SyntaxViolation::ExpectedFileDoubleSlash {
            invalid.set(true);
        }
    };
    let url = url::Url::options()
        .syntax_violation_callback(Some(&note_violation))
        .parse(line)
        .ok()?;
    if invalid.get() || url.query().is_some() || url.fragment().is_some() {
        return None;
    }
    url.to_file_path().ok()
}

/// `item`'s text with CRLF line endings made LF, as the synchronous text read always made them.
fn unix_line_endings(item: ClipboardItem) -> ClipboardItem {
    match item.text() {
        Some(text) if text.contains("\r\n") => {
            ClipboardItem::new_string(text.replace("\r\n", "\n"))
        }
        _ => item,
    }
}

/// A URI list with no local files supplies no attachment. A readable image or text supplies its usual payload.
fn decode_payload(
    bytes: Vec<u8>,
    file_list: bool,
    format: Option<ImageFormat>,
) -> Result<Option<ClipboardItem>, ClipboardReadError> {
    let item = if file_list {
        let paths = parse_file_uri_list(&bytes);
        if paths.0.is_empty() {
            return Ok(None);
        }
        ClipboardItem {
            entries: vec![ClipboardEntry::ExternalPaths(paths)],
        }
    } else if let Some(format) = format {
        let id = hash(&bytes);
        ClipboardItem::from(Image { format, bytes, id })
    } else {
        ClipboardItem::new_string(
            String::from_utf8(bytes).map_err(|_| ClipboardReadError::UnsupportedContent)?,
        )
    };
    Ok(Some(item))
}

impl Clipboard {
    pub fn new(connection: Connection, loop_handle: LoopHandle<'static, ()>) -> Self {
        Self {
            connection,
            loop_handle,
            self_mime: format!("pid/{}", std::process::id()),

            contents: None,
            primary_contents: None,

            current_offer: None,
            offer_generation: Rc::new(Cell::new(0)),
            cached_primary_read: None,
            current_primary_offer: None,
        }
    }

    pub fn set(&mut self, item: ClipboardItem) {
        self.contents = Some(item);
    }

    pub fn set_primary(&mut self, item: ClipboardItem) {
        self.primary_contents = Some(item);
    }

    pub fn set_offer(&mut self, data_offer: Option<DataOffer<WlDataOffer>>) {
        self.offer_generation
            .set(self.offer_generation.get().wrapping_add(1));
        self.current_offer = data_offer;
    }

    pub fn set_primary_offer(&mut self, data_offer: Option<DataOffer<ZwpPrimarySelectionOfferV1>>) {
        self.cached_primary_read = None;
        self.current_primary_offer = data_offer;
    }

    pub fn self_mime(&self) -> String {
        self.self_mime.clone()
    }

    pub fn send(&self, _mime_type: String, fd: OwnedFd) {
        if let Some(text) = self.contents.as_ref().and_then(|contents| contents.text()) {
            self.send_bytes(fd, text.as_bytes().to_owned());
        }
    }

    pub fn send_primary(&self, _mime_type: String, fd: OwnedFd) {
        if let Some(text) = self
            .primary_contents
            .as_ref()
            .and_then(|contents| contents.text())
        {
            self.send_bytes(fd, text.as_bytes().to_owned());
        }
    }

    /// Only this app's own copy is answered here. Another program's selection arrives through a pipe it writes when it
    /// gets around to it, and waiting for that on this thread holds the event loop and every window with it, so its
    /// contents come only from [`Clipboard::read_async`]. A text field's paste falls back to that read when this one
    /// gives nothing.
    pub fn read(&self) -> Option<ClipboardItem> {
        let offer = self.current_offer.as_ref()?;
        if offer.has_mime_type(&self.self_mime) {
            return self.contents.clone();
        }
        None
    }

    /// Keep the captured offer on the Wayland thread so fallback can request another flavor after the first pipe
    /// finishes. A selection change or loss of focus cancels the read even if the proxy still exists. The foreground
    /// task owns a proxy and connection clone, so it never holds a client borrow across the background read.
    pub fn read_async(
        &self,
        order: ReadOrder,
        background: &BackgroundExecutor,
        foreground: &ForegroundExecutor,
        has_focus: impl Fn() -> bool + 'static,
    ) -> Task<Result<Option<ClipboardItem>, ClipboardReadError>> {
        let Some(offer) = self.current_offer.as_ref() else {
            return Task::ready(Ok(None));
        };
        if offer.has_mime_type(&self.self_mime) {
            return Task::ready(Ok(self.contents.clone()));
        }
        let offer = offer.clone();
        let connection = self.connection.clone();
        let generation = self.offer_generation.clone();
        let captured = generation.get();
        let background = background.clone();
        foreground.spawn(async move {
            offer
                .read_payload(
                    order,
                    || generation.get() == captured && has_focus() && offer.inner.is_alive(),
                    |mime, format| offer.receive_payload(&connection, &background, mime, format),
                )
                .await
        })
    }

    pub fn read_primary(&mut self) -> Option<ClipboardItem> {
        let offer = self.current_primary_offer.as_ref()?;
        if let Some(cached) = self.cached_primary_read.clone() {
            return Some(cached);
        }

        if offer.has_mime_type(&self.self_mime) {
            return self.primary_contents.clone();
        }

        let item = offer
            .read_text(&self.connection)
            .or_else(|| offer.read_image(&self.connection))?;

        self.cached_primary_read = Some(item.clone());
        Some(item)
    }

    pub fn send_bytes(&self, fd: OwnedFd, bytes: Vec<u8>) {
        let mut written = 0;
        self.loop_handle
            .insert_source(
                calloop::generic::Generic::new(
                    File::from(fd),
                    calloop::Interest::WRITE,
                    calloop::Mode::Level,
                ),
                move |_, file, _| {
                    let file = unsafe { file.get_mut() };
                    loop {
                        match file.write(&bytes[written..]) {
                            Ok(n) if written + n == bytes.len() => {
                                written += n;
                                break Ok(PostAction::Remove);
                            }
                            Ok(n) => written += n,
                            Err(err) if err.kind() == ErrorKind::WouldBlock => {
                                break Ok(PostAction::Continue);
                            }
                            Err(_) => break Ok(PostAction::Remove),
                        }
                    }
                },
            )
            .unwrap();
    }
}
