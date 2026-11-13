// Modified for Farhelm: changed or added by the patch layers in vendor/patches/gpui/ (vendor/README.md, "GPUI").
use std::{
    fs::File,
    io::{ErrorKind, Write},
    os::fd::{AsRawFd, BorrowedFd, OwnedFd},
};

use calloop::{LoopHandle, PostAction};
use filedescriptor::Pipe;
use strum::IntoEnumIterator;
use wayland_client::{Connection, protocol::wl_data_offer::WlDataOffer};
use wayland_protocols::wp::primary_selection::zv1::client::zwp_primary_selection_offer_v1::ZwpPrimarySelectionOfferV1;

use crate::linux::platform::{PIPE_READ_TIMEOUT, read_fd_with_timeout};
use gpui::{
    BackgroundExecutor, ClipboardEntry, ClipboardItem, ClipboardReadError, Image, ImageFormat,
    Task, hash,
};

/// Text mime types that we'll offer to other programs.
pub(crate) const TEXT_MIME_TYPES: [&str; 3] =
    ["text/plain;charset=utf-8", "UTF8_STRING", "text/plain"];
pub(crate) const FILE_LIST_MIME_TYPE: &str = "text/uri-list";

/// Text mime types that we'll accept from other programs.
pub(crate) const ALLOWED_TEXT_MIME_TYPES: [&str; 2] = ["text/plain;charset=utf-8", "UTF8_STRING"];

pub(crate) struct Clipboard {
    connection: Connection,
    loop_handle: LoopHandle<'static, ()>,
    self_mime: String,

    // Internal clipboard
    contents: Option<ClipboardItem>,
    primary_contents: Option<ClipboardItem>,

    // External clipboard
    cached_read: Option<ClipboardItem>,
    current_offer: Option<DataOffer<WlDataOffer>>,
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

    /// Payload-oriented async reads choose an offered image before text. Keep
    /// the old synchronous editor API's ordering and cache independent. Aliases
    /// retain their actual offered MIME name when requesting the producer's data.
    fn payload_mime_type(&self) -> Option<(&str, Option<ImageFormat>)> {
        for format in ImageFormat::iter() {
            if let Some(mime) = self
                .mime_types
                .iter()
                .find(|mime| ImageFormat::from_mime_type(mime) == Some(format))
            {
                return Some((mime, Some(format)));
            }
        }
        TEXT_MIME_TYPES.iter().find_map(|preferred| {
            self.mime_types
                .iter()
                .find(|mime| mime.as_str() == *preferred)
                .map(|mime| (mime.as_str(), None))
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

impl Clipboard {
    pub fn new(connection: Connection, loop_handle: LoopHandle<'static, ()>) -> Self {
        Self {
            connection,
            loop_handle,
            self_mime: format!("pid/{}", std::process::id()),

            contents: None,
            primary_contents: None,

            cached_read: None,
            current_offer: None,
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
        self.cached_read = None;
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

    pub fn read(&mut self) -> Option<ClipboardItem> {
        let offer = self.current_offer.as_ref()?;
        if let Some(cached) = self.cached_read.clone() {
            return Some(cached);
        }

        if offer.has_mime_type(&self.self_mime) {
            return self.contents.clone();
        }

        let item = offer
            .read_text(&self.connection)
            .or_else(|| offer.read_image(&self.connection))?;

        self.cached_read = Some(item.clone());
        Some(item)
    }

    /// Issue receive/flush while the Wayland event-loop state is borrowed, then
    /// move only the pipe to the existing background executor. Neither the offer
    /// nor a client RefCell borrow crosses the await. This read does not update
    /// the synchronous cache: its image-first representation may differ, and a
    /// later selection must not inherit this captured offer's result.
    pub fn read_async(
        &self,
        executor: &BackgroundExecutor,
    ) -> Task<Result<Option<ClipboardItem>, ClipboardReadError>> {
        let Some(offer) = self.current_offer.as_ref() else {
            return Task::ready(Ok(None));
        };
        if offer.has_mime_type(&self.self_mime) {
            return Task::ready(Ok(self.contents.clone()));
        }
        let Some((mime, format)) = offer.payload_mime_type() else {
            return Task::ready(Ok(None));
        };
        let pipe = match Pipe::new() {
            Ok(pipe) => pipe,
            Err(error) => {
                log::error!("creating async clipboard pipe: {error}");
                return Task::ready(Err(ClipboardReadError::Unavailable));
            }
        };
        offer.inner.receive_data(mime.to_owned(), unsafe {
            BorrowedFd::borrow_raw(pipe.write.as_raw_fd())
        });
        let fd = pipe.read;
        drop(pipe.write);
        if let Err(error) = self.connection.flush() {
            log::error!("flushing async clipboard receive: {error}");
            return Task::ready(Err(ClipboardReadError::Unavailable));
        }
        executor.spawn(async move {
            // This uses the backend's existing per-poll timeout. Dropping the
            // GUI waiter prevents admission, but does not interrupt a blocking
            // read already executing on this worker thread.
            let bytes = read_fd_with_timeout(fd, PIPE_READ_TIMEOUT).map_err(|error| {
                log::error!("reading async clipboard pipe: {error}");
                ClipboardReadError::UnsupportedContent
            })?;
            let item = if let Some(format) = format {
                let id = hash(&bytes);
                ClipboardItem::from(Image { format, bytes, id })
            } else {
                ClipboardItem::new_string(
                    String::from_utf8(bytes).map_err(|_| ClipboardReadError::UnsupportedContent)?,
                )
            };
            Ok(Some(item))
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
