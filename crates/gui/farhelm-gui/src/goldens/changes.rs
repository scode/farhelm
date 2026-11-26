//! Produce the pixel evidence for the runner's static review page.
//!
//! Comparisons can save it from the images they already have in memory. Updates only need it for the pairs whose
//! captures changed against the manifest, so the runner sends those paths back after it has resolved the references.

use anyhow::Context as _;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ImageEncoder as _, RgbaImage};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{BufWriter, Write as _};
use std::path::{Path, PathBuf};

/// Required dimensions survive even when there is no meaningful pixel correspondence between the pictures.
#[derive(Serialize)]
struct Review {
    reference_size: (u32, u32),
    capture_size: (u32, u32),
    pixels: Option<usize>,
}

/// An independent receipt for one requested appearance. Compare can produce a difference and still have rendered
/// successfully. Update stops after saving an image and leaves the comparison to the Python runner.
#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(super) enum RenderOutcome {
    Matched,
    Different { reason: String },
    Rendered,
    Failed { reason: String },
}

impl RenderOutcome {
    /// Preserve the libtest failure summary for differences and errors, separately from the coverage receipts.
    pub(super) fn problem(&self) -> Option<&str> {
        match self {
            Self::Different { reason } | Self::Failed { reason } => Some(reason),
            Self::Matched | Self::Rendered => None,
        }
    }
}

/// Workers receive disjoint scene names. A final filename becomes readable only after the receipt is complete, so
/// interruption leaves that appearance missing rather than publishing part of a record.
pub(super) fn write_outcome(
    directory: &Path,
    picture: &Path,
    outcome: &RenderOutcome,
) -> anyhow::Result<()> {
    let stem = picture
        .file_stem()
        .context("outcome picture has no file stem")?;
    let temporary = directory.join(stem).with_extension("tmp");
    fs::write(&temporary, serde_json::to_vec(outcome)?)?;
    fs::rename(temporary, directory.join(stem).with_extension("json"))?;
    Ok(())
}

/// An opaque marker makes alpha-only differences visible. Reuse the old pixel storage for the highlighted copy.
fn review(mut reference: RgbaImage, capture: &RgbaImage) -> (Review, Option<RgbaImage>) {
    let mut info = Review {
        reference_size: reference.dimensions(),
        capture_size: capture.dimensions(),
        pixels: None,
    };
    if info.reference_size != info.capture_size {
        return (info, None);
    }
    let mut count = 0;
    // RGBA storage is packed. Walk whole pixels directly because this also runs in unoptimized test builds.
    let before: &mut [u8] = &mut reference;
    let current: &[u8] = capture;
    let before = before.as_chunks_mut::<4>().0;
    let current = current.as_chunks::<4>().0;
    for (old, new) in before.iter_mut().zip(current) {
        *old = if *old == *new {
            [new[0] / 3, new[1] / 3, new[2] / 3, new[3]]
        } else {
            count += 1;
            [255, 104, 42, 255]
        };
    }
    info.pixels = Some(count);
    (info, Some(reference))
}

/// Write metadata last so it can only advertise a finished highlight. A null record means the reference is missing or
/// failed to decode, and cannot be displayed. With no record at all, the runner can still display a known reference by itself.
pub(super) fn write(
    directory: &Path,
    picture: &Path,
    reference: Option<RgbaImage>,
    capture: &RgbaImage,
) -> anyhow::Result<()> {
    let stem = picture
        .file_stem()
        .context("review picture has no file stem")?;
    let (info, highlight) = if let Some(reference) = reference {
        let (info, highlight) = review(reference, capture);
        (Some(info), highlight)
    } else {
        (None, None)
    };
    fs::create_dir_all(directory)?;
    if let Some(highlight) = highlight {
        let mut writer = BufWriter::new(File::create(directory.join(stem).with_extension("png"))?);
        // Deflate level one compresses the long flat spans of a highlight. The fast preset can leave almost
        // raw-sized output without a row filter, making a large review directory slow to copy and open.
        PngEncoder::new_with_quality(&mut writer, CompressionType::Level(1), FilterType::NoFilter)
            .write_image(
                highlight.as_raw(),
                highlight.width(),
                highlight.height(),
                image::ExtendedColorType::Rgba8,
            )?;
        writer.flush()?;
    }
    fs::write(
        directory.join(stem).with_extension("json"),
        serde_json::to_vec(&info)?,
    )?;
    Ok(())
}

/// The runner gives each worker a disjoint part of the catalog and a common output directory.
#[derive(Deserialize)]
struct Batch {
    directory: PathBuf,
    pairs: Vec<(PathBuf, PathBuf)>,
}

/// Updates run this after rendering and reference lookup. It needs the image codec, with no app or display.
#[test]
#[ignore = "called by scripts/check-gui-goldens.py after an update"]
fn review_pairs() -> anyhow::Result<()> {
    let request = std::env::var_os("FARHELM_GOLDEN_DIFF_BATCH")
        .context("FARHELM_GOLDEN_DIFF_BATCH is unset")?;
    let batch: Batch = serde_json::from_slice(&fs::read(request)?)?;
    for (reference, capture) in batch.pairs {
        let result = (|| {
            let expected = match image::open(&reference) {
                Ok(image) => Some(image.into_rgba8()),
                Err(error) => {
                    eprintln!(
                        "could not decode reference for {}: {error}",
                        capture.display()
                    );
                    None
                }
            };
            let current = image::open(&capture)?.into_rgba8();
            write(&batch.directory, &capture, expected, &current)
        })();
        if let Err(error) = result {
            // Other pairs still have useful evidence if a reference cannot be decoded or a write fails.
            eprintln!(
                "could not make a pixel review for {}: {error:#}",
                capture.display()
            );
        }
    }
    Ok(())
}

/// Every channel counts, including alpha. A dimmed unchanged pixel and an opaque marker must stay distinguishable.
#[test]
fn highlight_counts_pixels_instead_of_channels() {
    use image::Rgba;

    let before = RgbaImage::from_fn(3, 1, |_, _| Rgba([90, 120, 150, 200]));
    let mut current = before.clone();
    current.put_pixel(1, 0, Rgba([90, 120, 150, 99]));
    current.put_pixel(2, 0, Rgba([1, 2, 3, 4]));
    let (info, marked) = review(before, &current);
    assert_eq!(info.pixels, Some(2));
    let expected = RgbaImage::from_fn(3, 1, |x, _| {
        if x == 0 {
            Rgba([30, 40, 50, 200])
        } else {
            Rgba([255, 104, 42, 255])
        }
    });
    assert_eq!(marked, Some(expected));
}

/// Matching bytes can still have different coordinates. Resize evidence only names the two dimensions.
#[test]
fn resized_pictures_have_no_highlight() {
    let (info, marked) = review(RgbaImage::new(1, 2), &RgbaImage::new(2, 1));
    assert_eq!(info.reference_size, (1, 2));
    assert_eq!(info.capture_size, (2, 1));
    assert_eq!(info.pixels, None);
    assert_eq!(marked, None);
}
