/**
 * Record a page as an H.264 MP4 through Playwright's screencast API
 * (docs/readme-video/SPEC.md, "Recording").
 *
 * Why not Playwright's own video recording: its encoder settings are fixed
 * (VP8 at about one megabit, no quality option), which smears text, and a
 * product whose main surface is a terminal is mostly text. The screencast
 * API instead hands over every frame as a JPEG with a timestamp, and this
 * module owns the encoding.
 *
 * Two properties of the screencast shape the design:
 *
 * - Frames arrive only when the page repaints. A still screen produces no
 *   frames for as long as it stays still, so the video's timeline cannot be
 *   "one frame per frame received"; it is rebuilt from the timestamps, each
 *   frame held on screen until the next one replaces it.
 * - Frames are delivered at the viewport's CSS-pixel size whatever the
 *   device scale factor (observed in headless Chromium, both shells, with
 *   Playwright 1.62), so the output resolution is the viewport size. A
 *   page has one screencast and its first client picks the frame size for
 *   everyone (Playwright's trace screenshots once made it 800x450), so every
 *   frame is checked against the viewport and a mismatch fails the run.
 *
 * Recording therefore writes distinct frames to a scratch directory with
 * their place on the output timeline, and encoding happens once, after the
 * choreography ends, from an ffconcat list that gives each frame its
 * duration. Encoding offline keeps the encoder off the page's critical path:
 * a slow preset can never make the browser drop or delay frames.
 *
 * `pause()`/`resume()` cut stretches out of the timeline (a status that has
 * to settle, a fleet that has to stage): the screen is frozen on the last
 * frame before the cut and continues with whatever the page shows at the
 * resume, with no gap. `mark()` names an instant on the output timeline;
 * after encoding, every mark is extracted as a still so the result can be
 * reviewed frame by frame without watching the whole video.
 */
import type { Page } from "@playwright/test";
import { spawn } from "node:child_process";
import { lstatSync, mkdirSync, mkdtempSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";

export interface RecorderOptions {
  /**
   * The MP4 to write. Its directory also receives `<name>.marks.json` and
   * `<name>-stills/` (an existing one is replaced only when the recorder can
   * tell it made it; see `ensureStillsReplaceable`), and, while recording, a
   * scratch directory of the recorder's own making (see `Recorder`'s
   * `frameDir`).
   */
  output: string;
  /** Output frame rate. Frames arriving faster than this collapse into the latest one per slot. */
  fps?: number;
  /** JPEG quality of the screencast frames, 0-100. */
  quality?: number;
  /** H.264 constant rate factor; lower is better and larger. 18 is visually lossless for UI. */
  crf?: number;
}

/** One named instant on the output timeline, as written to the marks file. */
export interface Mark {
  label: string;
  /** Seconds from the start of the output video. */
  at: number;
  /** The still extracted for this mark, relative to the marks file. */
  still: string;
}

interface Entry {
  file: string;
  /** Seconds on the output timeline at which this frame appears. */
  at: number;
  /** Capture time (epoch ms) of the picture the entry holds now; a slot merge can replace it. */
  ts: number;
}

/**
 * One finished cut, in wall-clock epoch ms. `resume` is the entry the cut
 * resumed on, if any; a frame captured inside the cut but delivered after
 * the resume replaces that entry's picture only if it is newer than the
 * picture the entry holds now (its `ts`), which a later slot merge may
 * already have advanced.
 */
interface Cut {
  from: number;
  to: number;
  resume: Entry | null;
}

/** How long {@link Recorder.start} waits for the screencast's first frame. */
const FIRST_FRAME_TIMEOUT_MS = 10_000;

/**
 * The pixel size a baseline or progressive JPEG declares in its start-of-frame
 * segment, or null if none is found. Enough of a parser to check the
 * screencast's frame size without decoding an image.
 */
function jpegSize(data: Buffer): { width: number; height: number } | null {
  let i = 2;
  while (i + 9 < data.length) {
    if (data[i] !== 0xff) return null;
    // Any number of 0xFF fill bytes may precede a marker.
    while (data[i + 1] === 0xff && i + 10 < data.length) i += 1;
    const marker = data[i + 1];
    // SOF0..SOF15, minus DHT (C4), JPG (C8), and DAC (CC), which share the range.
    if (marker >= 0xc0 && marker <= 0xcf && marker !== 0xc4 && marker !== 0xc8 && marker !== 0xcc) {
      return { height: data.readUInt16BE(i + 5), width: data.readUInt16BE(i + 7) };
    }
    i += 2 + data.readUInt16BE(i + 2);
  }
  return null;
}

/**
 * A running recording. Create with {@link Recorder.start}; every other
 * method is valid only until {@link Recorder.finish}.
 *
 * Frames are placed on the timeline by the time they were CAPTURED, not
 * the time the callback runs. Delivery lags capture, so a frame captured
 * just before `pause()` can arrive after it, and one captured during a cut
 * can arrive after `resume()`; routing by arrival would freeze the cut on a
 * picture from inside it, or start the resumed stretch on one from before.
 */
export class Recorder {
  /**
   * Scratch for the raw frames, made fresh for this recording by `start`;
   * of the directories beside the output, the only one the recorder removes
   * without checking what is in it. Its name is chosen by `mkdtemp` rather
   * than derived from the output path, because a derived name could be a
   * directory the maintainer already had there, and removing it would delete
   * their files. It sits beside the output rather than under the system
   * temporary directory, which may be a small memory-backed filesystem, and
   * a long recording's frames are large. Its name is not hidden, so one left
   * by a run killed outright is easy to spot.
   */
  private readonly frameDir: string;
  private readonly entries: Entry[] = [];
  private readonly marks: Array<{ label: string; at: number }> = [];
  private readonly cuts: Cut[] = [];
  private readonly slot: number;
  /** Wall-clock epoch ms of output time zero. */
  private origin = 0;
  /** Wall-clock epoch ms the current pause began, or null while recording. */
  private pausedAt: number | null = null;
  /** The newest frame captured during the current pause, entered on the timeline at resume. */
  private held: { data: Buffer; ts: number } | null = null;
  /** The viewport size every frame must have; see the module docs for why it is checked. */
  private expected: { width: number; height: number } | null = null;
  /** The first frame-size violation, reported by `start` or `finish`. */
  private failure: string | null = null;
  private written = 0;
  private stopped = false;

  private constructor(
    private readonly page: Page,
    private readonly options: Required<RecorderOptions>,
    frameDir: string,
  ) {
    this.frameDir = frameDir;
    this.slot = 1 / options.fps;
  }

  /**
   * Start recording `page`. Resolves once the first frame has arrived, so
   * output time zero is a real picture rather than whatever an encoder
   * would invent before one.
   *
   * Fails if the screencast API is missing (it arrived in Playwright 1.59;
   * e2e's lockfile pins 1.62 while `package.json` still admits older
   * versions), if no frame arrives within {@link FIRST_FRAME_TIMEOUT_MS},
   * if the first frame is not the viewport's size, or if `<name>-stills/`
   * exists but is not one the recorder may replace.
   */
  static async start(page: Page, options: RecorderOptions): Promise<Recorder> {
    if (typeof (page as any).screencast?.start !== "function") {
      throw new Error("page.screencast is missing: the recorder needs Playwright 1.59 or newer (e2e/package-lock.json pins it)");
    }
    const settings = { fps: 30, quality: 92, crf: 18, ...options };
    // Checked here as well as before the stills are written, so a directory
    // the recorder may not replace stops the run before a long recording
    // rather than after it.
    ensureStillsReplaceable(stillsDir(settings.output));
    const outputDir = path.dirname(settings.output);
    mkdirSync(outputDir, { recursive: true });
    const frameDir = mkdtempSync(path.join(outputDir, `${path.basename(settings.output)}.frames-`));
    const recorder = new Recorder(page, settings, frameDir);
    let first: () => void = () => {};
    const firstFrame = new Promise<void>((resolve) => (first = resolve));
    const viewport = page.viewportSize();
    recorder.expected = viewport;
    let timer: NodeJS.Timeout | undefined;
    try {
      await page.screencast.start({
        quality: recorder.options.quality,
        ...(viewport ? { size: viewport } : {}),
        onFrame: ({ data, timestamp }) => {
          if (recorder.origin === 0) {
            recorder.origin = timestamp;
            first();
          }
          recorder.frame(data, timestamp);
        },
      });
      // A page that is completely still may not repaint on its own; a no-op
      // style flip on the root forces the one paint the screencast needs.
      await page.evaluate(() => {
        document.documentElement.style.outline = "0 solid transparent";
        requestAnimationFrame(() => document.documentElement.style.removeProperty("outline"));
      });
      const timeout = new Promise<never>((_, reject) => {
        timer = setTimeout(
          () => reject(new Error(`the screencast delivered no frame within ${FIRST_FRAME_TIMEOUT_MS} ms`)),
          FIRST_FRAME_TIMEOUT_MS,
        );
      });
      await Promise.race([firstFrame, timeout]);
      if (recorder.failure) throw new Error(recorder.failure);
    } catch (error) {
      // Leave nothing running or on disk behind a recording that never
      // started. Every failure after the scratch directory exists lands
      // here: each run makes a new one, so a leaked one would pile up.
      await recorder.discard();
      throw error;
    } finally {
      clearTimeout(timer);
    }
    return recorder;
  }

  /** Total ms of finished cuts that ended at or before `epochMs`. */
  private cutBefore(epochMs: number): number {
    return this.cuts.filter((cut) => cut.to <= epochMs).reduce((sum, cut) => sum + cut.to - cut.from, 0);
  }

  /**
   * Map a wall-clock epoch-ms instant onto the output timeline, in seconds.
   * An instant inside a cut, finished or ongoing, maps to the cut's start,
   * which is also where the timeline resumes.
   */
  private outputTime(epochMs: number): number {
    const inside = this.cuts.find((cut) => epochMs >= cut.from && epochMs < cut.to);
    let at = inside ? inside.from : epochMs;
    if (this.pausedAt !== null && at >= this.pausedAt) at = this.pausedAt;
    return Math.max(0, (at - this.origin - this.cutBefore(at)) / 1000);
  }

  /**
   * Route one screencast frame by its capture time: held aside if captured
   * during the current pause, used to refresh a finished cut's resume
   * picture if captured inside that cut, otherwise put on the timeline.
   */
  private frame(data: Buffer, timestamp: number): void {
    if (this.stopped) return;
    const size = jpegSize(data);
    if (this.expected && (!size || size.width !== this.expected.width || size.height !== this.expected.height)) {
      // Another screencast client can pick the frame size for the whole
      // page (docs/readme-video/SPEC.md, "Recording"); a video quietly
      // recorded at the wrong resolution is the failure this guards.
      this.failure ??= `screencast frame is ${size ? `${size.width}x${size.height}` : "of unknown size"}, ` +
        `expected the viewport's ${this.expected.width}x${this.expected.height}`;
      return;
    }
    if (this.pausedAt !== null && timestamp >= this.pausedAt) {
      if (!this.held || timestamp >= this.held.ts) this.held = { data, ts: timestamp };
      return;
    }
    const cut = this.cuts.find((c) => timestamp >= c.from && timestamp < c.to);
    if (cut) {
      if (cut.resume) {
        if (timestamp <= cut.resume.ts) return;
        writeFileSync(cut.resume.file, data);
        cut.resume.ts = timestamp;
      } else {
        cut.resume = this.enter(data, this.outputTime(cut.to), timestamp);
      }
      return;
    }
    this.enter(data, this.outputTime(timestamp), timestamp);
  }

  /**
   * Put a frame on the timeline at `at` and return the entry that now holds
   * it, or null if it was dropped. A frame landing within the same output
   * slot as the previous one replaces it: the encoder could only show one of
   * them, and keeping the newest keeps the picture current. A frame that
   * would land before the previous one is stale and is dropped, so the
   * timeline never runs backwards.
   */
  private enter(data: Buffer, at: number, ts: number): Entry | null {
    const last = this.entries.at(-1);
    if (last && at < last.at) return null;
    if (last && at - last.at < this.slot) {
      writeFileSync(last.file, data);
      last.ts = ts;
      return last;
    }
    const file = path.join(this.frameDir, `${String(this.written++).padStart(6, "0")}.jpg`);
    writeFileSync(file, data);
    const entry = { file, at, ts };
    this.entries.push(entry);
    return entry;
  }

  /** Stop advancing the timeline until {@link resume}; the viewer sees no gap. */
  pause(): void {
    if (this.pausedAt === null) this.pausedAt = Date.now();
  }

  /** Continue the timeline from where {@link pause} froze it, with the page's current picture. */
  resume(): void {
    if (this.pausedAt === null) return;
    const cut: Cut = { from: this.pausedAt, to: Date.now(), resume: null };
    this.cuts.push(cut);
    this.pausedAt = null;
    if (this.held) {
      cut.resume = this.enter(this.held.data, this.outputTime(cut.to), this.held.ts);
      this.held = null;
    }
  }

  /** Name the current instant for review; returns its output time in seconds. */
  mark(label: string): number {
    const at = this.outputTime(this.pausedAt ?? Date.now());
    this.marks.push({ label, at });
    return at;
  }

  /**
   * Stop the screencast, encode the MP4, extract a still per mark, and
   * remove the scratch frames, whether or not the rest succeeded: a failed
   * recording cannot be encoded again, and each run makes a new scratch
   * directory, so one left behind would only pile up. Returns the marks
   * with their still paths.
   */
  async finish(): Promise<Mark[]> {
    const end = this.outputTime(this.pausedAt ?? Date.now());
    this.stopped = true;
    try {
      return await this.encode(end);
    } finally {
      this.removeScratch();
    }
  }

  /**
   * Abandon the recording: stop the screencast and remove the frame scratch,
   * encoding nothing. Safe to call more than once and after {@link finish},
   * which is what lets a caller run it unconditionally from teardown: a beat
   * that fails between `start` and `finish` would otherwise leave the
   * scratch behind, and since every run makes a new directory, nothing
   * would ever remove it.
   */
  async discard(): Promise<void> {
    if (!this.stopped) {
      this.stopped = true;
      await this.page.screencast.stop().catch(() => {});
    }
    this.removeScratch();
  }

  /**
   * Remove the frame scratch, warning rather than throwing if that fails,
   * so a cleanup problem never replaces the error a caller is reporting.
   */
  private removeScratch(): void {
    try {
      rmSync(this.frameDir, { recursive: true, force: true });
    } catch (error) {
      console.warn(`could not remove the recorder's frame scratch ${this.frameDir}: ${error}`);
    }
  }

  /** {@link finish}'s work once the timeline has ended at `end` seconds. */
  private async encode(end: number): Promise<Mark[]> {
    await this.page.screencast.stop();
    if (this.failure) throw new Error(this.failure);
    if (this.entries.length === 0) throw new Error("the recording captured no frames");

    // ffconcat: each frame with how long it stays on screen. The demuxer
    // ignores the last entry's duration unless the file is listed once
    // more, which is the documented idiom for holding the final frame. Paths
    // are single-quoted with the shell's `'\''` idiom, which the demuxer's
    // tokenizer also understands, so an --output path with a quote survives.
    const quote = (file: string) => `'${file.replaceAll("'", "'\\''")}'`;
    const lines = ["ffconcat version 1.0"];
    for (const [index, entry] of this.entries.entries()) {
      const until = this.entries[index + 1]?.at ?? Math.max(end, entry.at + this.slot);
      lines.push(`file ${quote(entry.file)}`, `duration ${(until - entry.at).toFixed(6)}`);
    }
    lines.push(`file ${quote((this.entries.at(-1) as Entry).file)}`);
    const list = path.join(this.frameDir, "frames.ffconcat");
    writeFileSync(list, `${lines.join("\n")}\n`);

    const { output, fps, crf } = this.options;
    // Checked again before the encode overwrites the MP4: a refusal after it
    // would leave a new video beside the previous recording's marks and
    // stills, which no longer describe it.
    ensureStillsReplaceable(stillsDir(output));
    await run("ffmpeg", [
      "-hide_banner", "-loglevel", "error", "-y",
      "-f", "concat", "-safe", "0", "-i", list,
      // Constant frame rate output from the variable-rate input; yuv420p and
      // faststart are what every player and upload pipeline expects.
      "-vf", `fps=${fps}`,
      "-c:v", "libx264", "-preset", "slow", "-crf", String(crf),
      "-pix_fmt", "yuv420p", "-movflags", "+faststart",
      output,
    ]);

    const base = output.replace(/\.mp4$/, "");
    const stillDir = stillsDir(output);
    rmSync(stillDir, { recursive: true, force: true });
    mkdirSync(stillDir, { recursive: true });
    const marks: Mark[] = [];
    for (const [index, mark] of this.marks.entries()) {
      const slug = mark.label.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
      const still = path.join(stillDir, `${String(index + 1).padStart(2, "0")}-${slug}.png`);
      // Seek a hair past the mark so the still shows the frame the mark
      // names rather than the one before it.
      await run("ffmpeg", [
        "-hide_banner", "-loglevel", "error", "-y",
        "-ss", (mark.at + 0.05).toFixed(3), "-i", output, "-frames:v", "1", still,
      ]);
      marks.push({ label: mark.label, at: Number(mark.at.toFixed(3)), still: path.relative(path.dirname(output), still) });
    }
    writeFileSync(`${base}.marks.json`, `${JSON.stringify({ duration: Number(end.toFixed(3)), marks }, null, 2)}\n`);
    return marks;
  }
}

/**
 * Where the review stills for `output` go: `<name>-stills/` beside the MP4,
 * where docs/readme-video/SPEC.md and the refresh procedure look for them.
 */
function stillsDir(output: string): string {
  return `${output.replace(/\.mp4$/, "")}-stills`;
}

/**
 * The name `finish` gives a still: the mark's position, at least two
 * digits, then the label's slug, which is lowercase letters, digits and
 * dashes, possibly empty.
 */
const STILL_NAME = /^\d{2,}-[a-z0-9-]*\.png$/;

/**
 * Files an operating system's file browser leaves in a folder someone looked
 * at (macOS Finder, Windows Explorer). Reviewing the stills that way is the
 * refresh procedure, so their presence must not make an earlier recording's
 * stills unreplaceable; they hold nothing anyone would miss.
 */
const BROWSER_LITTER = new Set([".DS_Store", "Thumbs.db", "desktop.ini"]);

/**
 * Throw unless `dir` is absent or a directory holding nothing but files
 * named the way the recorder names stills (and file-browser litter), which
 * is what the recorder may replace wholesale.
 *
 * The stills directory's name is derived from the output path, so it can be
 * a directory the maintainer made for something else, and replacing it
 * would delete their files. Recognizing the recorder's own file names, rather
 * than a marker file written on creation, keeps stills directories from
 * recordings made before this check replaceable without any migration. A
 * symlink is refused rather than followed, and so is anything else that is
 * not a plain directory.
 */
function ensureStillsReplaceable(dir: string): void {
  const refuse = (what: string): never => {
    throw new Error(
      `the stills directory ${dir} ${what}; move it aside and record again ` +
        "(the recorder only replaces a stills directory of its own)",
    );
  };
  let stat;
  try {
    stat = lstatSync(dir);
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return;
    throw error;
  }
  if (!stat.isDirectory()) refuse("is not a plain directory (a symlink, or not a directory at all)");
  const foreign = readdirSync(dir, { withFileTypes: true })
    .filter((entry) => !(entry.isFile() && (STILL_NAME.test(entry.name) || BROWSER_LITTER.has(entry.name))))
    .map((entry) => entry.name)
    .sort();
  if (foreign.length > 0) {
    const more = foreign.length > 3 ? ` and ${foreign.length - 3} more` : "";
    refuse(`holds ${foreign.slice(0, 3).join(", ")}${more}, which the recorder did not make`);
  }
}

/** Run a command to completion, failing with its stderr. */
function run(command: string, args: string[]): Promise<void> {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, { stdio: ["ignore", "ignore", "pipe"] });
    let stderr = "";
    child.stderr.on("data", (chunk) => (stderr += chunk));
    child.on("error", reject);
    child.on("close", (code) => code === 0 ? resolve() : reject(new Error(`${command} exited ${code}: ${stderr}`)));
  });
}
