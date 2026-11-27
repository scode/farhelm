//! What a copy-contents click on a file link does: read the file's bytes from its host into memory, decide what they
//! are, and put them on the clipboard, saying how it went on the status line at the top of the terminal (SPEC.md,
//! "Terminal experience", the file-path bullet).
//!
//! NOTE: The contents go to the clipboard only as one of a short, explicit list of things: text (valid UTF-8 with no
//! NUL byte, which covers source, logs with escape sequences, and SVG), or, on macOS, a raster image whose extension is
//! on [`IMAGE_EXTENSIONS`] and whose first bytes are that format's signature. Nothing is ever guessed: an image
//! extension whose bytes are not that image is refused rather than copied as text, and anything else (a PDF, an
//! archive, AVIF, an executable) is refused with words saying so. The maintainer asked for exactly this: "a clear, loud
//! list of specific formats that make sense" (the plan's decisions). Linux copies text only for now, because its
//! clipboard backend in GPUI writes text alone; an image there is refused before any bytes move.
//!
//! The bytes come through the helm's download route asked to relay rather than save
//! ([`farhelm_helm_client::Transfers::download_bytes`]), so nothing lands in Downloads, capped at [`COPY_MAX_BYTES`].
//! The status line follows the download's: "Copying…" while the read runs, "Copied to clipboard" (fading as a save's
//! success does), or "Copy failed: …" until the next press. "Copied" means Farhelm handed the contents to the system
//! clipboard; whether the system kept them is outside Farhelm's control (SPEC.md's best-effort clipboard rule, which
//! includes Wayland dropping a write from an unfocused window, and an empty text being no write at all there).

use farhelm_helm_client::TransferError;
use farhelm_input_mapping::Platform;
use gpui::{ClipboardItem, Context, Image, ImageFormat};

use super::download::{Leaves, SENTENCE_ID};
use super::{FileLinks, failure, format_size};
use crate::host_text::{HostSentence, HostText};

/// The largest file a copy reads: 10 MB in decimal bytes, like the host's own download limit, so the hover's sizes and
/// this cap use one unit. A clipboard is not where a bigger file belongs.
pub(in crate::terminal) const COPY_MAX_BYTES: u64 = 10_000_000;

/// Why an image cannot be copied on Linux, whether the hover's path or the read's says so.
const LINUX_IMAGE_REFUSAL: &str = "only text can be copied here; images can be copied on macOS";

/// The raster image extensions a copy recognizes, lower case. Each names one of GPUI's clipboard image formats
/// ([`image_format`]); SVG is not here because it is text and copies as such, and AVIF is not because GPUI cannot put it
/// on the clipboard.
const IMAGE_EXTENSIONS: [&str; 9] = [
    "png", "jpg", "jpeg", "webp", "gif", "bmp", "tif", "tiff", "ico",
];

/// What a copy puts on the clipboard.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Clip {
    Text(String),
    Image(Image),
}

/// The final extension of `path`, lower case, as the helm reads one when it decides what may open (`Path::extension`:
/// a dotfile such as `.png` has none).
fn extension(path: &str) -> Option<String> {
    std::path::Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
}

/// Whether `path` names a raster image a copy recognizes by its extension.
pub(super) fn is_image_path(path: &str) -> bool {
    extension(path).is_some_and(|extension| IMAGE_EXTENSIONS.contains(&extension.as_str()))
}

/// The clipboard image format an [`IMAGE_EXTENSIONS`] entry names.
fn image_format(extension: &str) -> Option<ImageFormat> {
    Some(match extension {
        "png" => ImageFormat::Png,
        "jpg" | "jpeg" => ImageFormat::Jpeg,
        "webp" => ImageFormat::Webp,
        "gif" => ImageFormat::Gif,
        "bmp" => ImageFormat::Bmp,
        "tif" | "tiff" => ImageFormat::Tiff,
        "ico" => ImageFormat::Ico,
        _ => return None,
    })
}

/// Whether `bytes` start with `format`'s signature: the fixed bytes every file of that format begins with. This proves
/// the file claims to be that format, which is all the clipboard needs; it does not decode the image.
fn has_signature(format: ImageFormat, bytes: &[u8]) -> bool {
    match format {
        ImageFormat::Png => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        ImageFormat::Jpeg => bytes.starts_with(b"\xff\xd8\xff"),
        ImageFormat::Gif => bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"),
        ImageFormat::Webp => {
            bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP"
        }
        ImageFormat::Bmp => bytes.starts_with(b"BM"),
        ImageFormat::Tiff => bytes.starts_with(b"II*\0") || bytes.starts_with(b"MM\0*"),
        ImageFormat::Ico => bytes.starts_with(b"\0\0\x01\0"),
        // Never reached: `image_format` names none of these, so no copy is ever checked against them.
        ImageFormat::Svg | ImageFormat::Pnm => false,
    }
}

/// Why a copy on `platform` of `path` cannot start, known before any bytes move: an image on Linux, where only text
/// copies, and a file the hover already showed to be over [`COPY_MAX_BYTES`]. `None` lets the copy read the file.
pub(super) fn refused_before_reading(
    path: &str,
    size: Option<u64>,
    platform: Platform,
) -> Option<String> {
    if platform == Platform::Linux && is_image_path(path) {
        return Some(LINUX_IMAGE_REFUSAL.into());
    }
    match size {
        Some(size) if size > COPY_MAX_BYTES => Some(too_large()),
        _ => None,
    }
}

/// The refusal of a file over the cap, in the hover's own size units.
fn too_large() -> String {
    format!("the file is larger than {}", format_size(COPY_MAX_BYTES))
}

/// What the clipboard gets for `bytes`, the contents of `path` on its host, copied on `platform`; or why nothing.
///
/// An image extension decides the kind before the bytes do, so a `.png` that is not a PNG is refused, never copied as
/// whatever its bytes look like. Everything else must be text. Pure, so that the whole list is tested without a
/// clipboard.
pub(super) fn classify(path: &str, bytes: Vec<u8>, platform: Platform) -> Result<Clip, String> {
    if let Some(format) = extension(path).as_deref().and_then(image_format) {
        if platform == Platform::Linux {
            return Err(LINUX_IMAGE_REFUSAL.into());
        }
        if !has_signature(format, &bytes) {
            return Err(format!(
                "the file is named as {} but its contents are not one",
                format_name(format)
            ));
        }
        return Ok(Clip::Image(Image::from_bytes(format, bytes)));
    }
    if bytes.contains(&0) {
        return Err("the file is not text or a supported image".into());
    }
    String::from_utf8(bytes)
        .map(Clip::Text)
        .map_err(|_| "the file is not text or a supported image".into())
}

/// A format's name in a refusal, as the user knows it.
fn format_name(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Png => "a PNG image",
        ImageFormat::Jpeg => "a JPEG image",
        ImageFormat::Webp => "a WebP image",
        ImageFormat::Gif => "a GIF image",
        ImageFormat::Bmp => "a BMP image",
        ImageFormat::Tiff => "a TIFF image",
        ImageFormat::Ico => "an icon",
        ImageFormat::Svg => "an SVG image",
        ImageFormat::Pnm => "a Netpbm image",
    }
}

/// Why a copy failed, by whose words say so: the app's own (a refusal before reading, the cap, the classifier), drawn
/// as app text, or the helm's or host's, drawn as host text so that hidden characters in them show.
enum Reason {
    Ours(String),
    Hosts(HostText),
}

impl FileLinks {
    /// Copy the contents of `path` (the host's resolved path, which the host reopens) into the clipboard. `size` is the
    /// hover's size, which refuses an oversized file before anything is read; the read enforces the cap again, since
    /// the file may have grown since. `platform` is the hover's, which decides whether images copy.
    ///
    /// Only the latest copy may reach the clipboard: starting one drops the copy still in flight, which abandons its
    /// read, so a large file asked for first can never land over a small one asked for after it.
    pub(super) fn copy(
        &mut self,
        path: String,
        size: Option<u64>,
        platform: Platform,
        cx: &mut Context<Self>,
    ) {
        if let Some(reason) = refused_before_reading(&path, size, platform) {
            self.copying = None;
            self.copy_failed(Reason::Ours(reason), cx);
            return;
        }
        // A check succeeded, so the session has transfers; without them there is nothing to ask.
        let Some(transfers) = self.transfers.clone() else {
            return;
        };
        // The cap fits `usize` on every platform the app builds for.
        let reading = transfers.download_bytes(&self.session, &path, COPY_MAX_BYTES as usize);
        // Replacing the task drops the earlier copy's, as dropping the file links drops this one: either abandons the
        // read. A copy that gets here is therefore still the latest.
        self.copying = Some(cx.spawn(async move |files, cx| {
            let read = reading.await;
            let _ = files.update(cx, |files, cx| {
                files.copying = None;
                files.read(&path, read, platform, cx);
            });
        }));
        self.show(
            HostSentence::new(SENTENCE_ID).text("Copying…"),
            Leaves::Replaced,
            cx,
        );
    }

    /// A copy's read finished: put what it read on the clipboard, or say why not.
    fn read(
        &mut self,
        path: &str,
        read: Result<Vec<u8>, TransferError>,
        platform: Platform,
        cx: &mut Context<Self>,
    ) {
        let bytes = match read {
            Ok(bytes) => bytes,
            Err(TransferError::TooLarge { .. }) => {
                return self.copy_failed(Reason::Ours(too_large()), cx);
            }
            Err(error) => {
                return self.copy_failed(Reason::Hosts(failure(&error, "Copy refused")), cx);
            }
        };
        let item = match classify(path, bytes, platform) {
            Ok(Clip::Text(text)) => ClipboardItem::new_string(text),
            Ok(Clip::Image(image)) => ClipboardItem::new_image(&image),
            Err(reason) => return self.copy_failed(Reason::Ours(reason), cx),
        };
        // Best effort, as every clipboard write in the app is (SPEC.md): GPUI reports no failure to report here.
        cx.write_to_clipboard(item);
        self.show(
            HostSentence::new(SENTENCE_ID).text("Copied to clipboard"),
            Leaves::Fading,
            cx,
        );
    }

    /// Say why a copy failed, until the next press, as a failed save does.
    fn copy_failed(&mut self, reason: Reason, cx: &mut Context<Self>) {
        let line = HostSentence::new(SENTENCE_ID).text("Copy failed: ");
        let line = match reason {
            Reason::Ours(words) => line.text(words),
            Reason::Hosts(words) => line.host(&words),
        };
        self.show(line, Leaves::AtPress, cx);
    }
}
