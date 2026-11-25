"""Build the golden runner's portable review page from captures left by the renderer."""

from __future__ import annotations

import fcntl
import html
import json
import shutil
import sys
import tempfile
from collections.abc import Callable
from pathlib import Path

PAGE = """<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>Golden screenshot review</title>
<style>
body {{ font: 14px system-ui, sans-serif; margin: 16px; background: #f4f5f8; color: #1a1d24; }}
section {{ background: #fff; border: 1px solid #d0d4dc; border-radius: 6px; margin: 12px 0; padding: 8px 12px; }}
h2 {{ font-size: 1.15rem; overflow-wrap: anywhere; }}
h2 span, figcaption {{ color: #5a6070; }}
.pictures {{ display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 12px; align-items: start; }}
figure {{ margin: 0; }} figcaption {{ margin-bottom: 4px; }}
img {{ max-width: 100%; height: auto; box-sizing: border-box; border: 1px solid #ccc; }}
.missing {{ color: #a33; }}
@media (max-width: 700px) {{ .pictures {{ grid-template-columns: 1fr; }} }}
</style></head><body>
<h1>Golden screenshot review</h1>
{incomplete}
{warning}
{rows}
</body></html>
"""

ReviewBatch = Callable[[Path, list[tuple[Path, Path]], Path], None]


def figure(name: str, label: str) -> str:
    """Link to the full-size file so a reviewer can inspect small differences at native scale."""
    name, label = html.escape(name), html.escape(label)
    return (f'<figure><figcaption>{label}</figcaption><a href="{name}">'
            f'<img src="{name}" alt="{label}"></a></figure>')


def appearance_heading(stem: str) -> str:
    """Name the appearance consistently in the failures list and image cards."""
    scene, appearance = stem.rsplit("--", 1)
    return f"<code>{html.escape(scene)}</code> <span>· {html.escape(appearance)}</span>"


def incomplete_section(incomplete: dict[str, str]) -> str:
    """A missing render must stay visible even when there are no changed pictures to inspect."""
    if not incomplete:
        return ""
    rows = "".join(
        f"<li>{appearance_heading(stem)} · {html.escape(reason)}</li>"
        for stem, reason in incomplete.items()
    )
    return f'<section><h2>Missing or failed renders</h2><ul class="missing">{rows}</ul></section>'


def picture_row(out: Path, stem: str, has_reference: bool, new_scene: bool, reviews: Path) -> tuple[str, bool]:
    """Use the native test's pixel evidence when present. A pair can still be inspected without its highlight."""
    captured = figure(f"{stem}-capture.png", "New capture")
    original = f"{stem}-reference.png"
    review = {}
    if has_reference:
        try:
            review = json.loads((reviews / f"{stem}.json").read_text(encoding="utf-8"))
        except (OSError, ValueError):
            pass
        if review is None:
            # A native decoder failure is distinct from a missing highlight. Avoid packaging a broken reference.
            (out / original).unlink()
            has_reference = False
    if not has_reference:
        label = "New scene" if new_scene else "Reference unavailable"
        previous, diff = f'<p class="missing">{label}</p>', ""
    else:
        previous = figure(original, "Reference")
        if not review:
            diff = '<p class="missing">Pixel comparison unavailable.</p>'
        elif review["pixels"] is None:
            old_w, old_h = review["reference_size"]
            width, height = review["capture_size"]
            previous = figure(original, f"Reference ({old_w} × {old_h})")
            captured = figure(f"{stem}-capture.png", f"New capture ({width} × {height})")
            diff = "<p>Image size changed.</p>"
        else:
            marked = f"{stem}-diff.png"
            shutil.copyfile(reviews / f"{stem}.png", out / marked)
            count = review["pixels"]
            noun = "pixel" if count == 1 else "pixels"
            diff = figure(marked, f"{count:,} differing {noun}")
    heading = appearance_heading(stem)
    row = f'<section><h2>{heading}</h2><div class="pictures">{previous}{captured}{diff}</div></section>'
    return row, not has_reference and not new_scene


def copy_reference(tool, reference: Path, target: Path, expected_blob: str | None) -> bool:
    """Use one copy of the reference for both the displayed image and the update batch's pixel comparison."""
    try:
        data = reference.read_bytes()
    except OSError:
        return False
    # A local update baseline can predate the manifest. Compare uses whatever reference the renderer read.
    if expected_blob is not None and tool.blob_id(data) != expected_blob:
        return False
    target.write_bytes(data)
    return True


def report(tool, captures: Path, platform: str, references: Path | None, base_env: dict[str, str],
           review_batch: ReviewBatch, manifest: Path, incomplete: dict[str, str]) -> Path | None:
    """Replace the checkout's review directory after each render, without changing the renderer's result.

    Compare mode has already filtered by pixel equality. Update mode writes the full selected set, so use the
    publisher's byte identity and fetch references only if at least one existing picture changed.
    """
    dest = tool.WORK / "changes"
    try:
        with tempfile.TemporaryDirectory(prefix=".changes-", dir=tool.WORK) as temp:
            out = Path(temp) / "page"
            ready = False
            try:
                collect_page(tool, Path(temp), out, captures, platform, references, base_env, review_batch, manifest,
                             incomplete)
                ready = (out / "index.html").is_file()
            finally:
                # Overlapping comparisons own their own captures. Serialize just the shared directory replacement.
                # Even if page creation failed, an older page must not be mistaken for this render's results.
                with dest.with_name(".changes.lock").open("a") as lock:
                    fcntl.flock(lock, fcntl.LOCK_EX)
                    if dest.exists():
                        shutil.rmtree(dest)
                    if ready:
                        out.rename(dest)
    except (tool.Refused, OSError, ValueError) as error:
        print(f"could not write golden changes page: {error}", file=sys.stderr)
        return None
    return dest / "index.html" if ready else None


def collect_page(tool, temp: Path, out: Path, captures: Path, platform: str, references: Path | None,
                 base_env: dict[str, str], review_batch: ReviewBatch, manifest: Path, incomplete: dict[str, str]) -> None:
    """Use completed captures. A failed writer may leave a partial file that belongs only in the run evidence."""
    pictures = sorted(p for p in (captures / platform).glob("*--*.png") if p.stem not in incomplete)
    if not pictures and not incomplete:
        return
    entries = tool.read_manifest(manifest)
    ids = {picture: entries.get(f"{platform}/{picture.name}") for picture in pictures}
    updating = references is None
    local = base_env.get("FARHELM_GOLDENS_DIR") if updating else None
    if updating:
        ids = {picture: blob for picture, blob in ids.items()
               if blob is None or tool.blob_id(picture.read_bytes()) != blob}
        if not ids and not incomplete:
            return
        if local:
            references = Path(local).resolve()
        elif any(ids.values()):
            # The store's fetch takes a manifest. Give it just the older images this page needs, particularly when
            # --scene selected a small part of a large catalog.
            subset = temp / "goldens.txt"
            try:
                tool.write_manifest(subset, {f"{platform}/{p.name}": blob for p, blob in ids.items() if blob is not None})
                references = tool.fetch(subset, temp / "refs", tool.REMOTE, tool.CACHE)
            except (tool.Refused, OSError) as error:
                print(f"could not fetch references for the golden changes page: {error}", file=sys.stderr)

    # Compare has already produced highlights alongside its failed captures. Updates can do that work only after
    # their manifest filter and reference lookup, and should never leave review files in the publish directory.
    reviews = temp / "review" if updating else captures / platform / "review"
    out.mkdir()
    pictures, pairs = [], []
    for picture, blob in ids.items():
        ref = references / platform / picture.name if references is not None and (not updating or blob) else None
        original = out / f"{picture.stem}-reference.png"
        paired = ref is not None and copy_reference(tool, ref, original, blob if local else None)
        shutil.copyfile(picture, out / f"{picture.stem}-capture.png")
        pictures.append((picture.stem, paired, blob is None))
        if paired and updating:
            pairs.append((original, picture))
    if pairs:
        review_batch(temp, pairs, reviews)
    rows = [picture_row(out, *picture, reviews) for picture in pictures]
    warning = ('<p class="missing">Some reference pictures are unavailable.</p>'
               if any(unavailable for _, unavailable in rows) else "")
    (out / "index.html").write_text(
        PAGE.format(incomplete=incomplete_section(incomplete), warning=warning, rows="\n".join(row for row, _ in rows)),
        encoding="utf-8",
    )
