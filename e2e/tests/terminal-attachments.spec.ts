// ---------------------------------------------------------------------
// Paste and drop interception (PLAN_M4.md item 7).
//
// SPEC.md's attachments contract, end to end: a file dropped on ANY of a
// session's terminals is transferred to the session's host and its
// host-side path inserted at that terminal's cursor, a pasted image does
// the same under a generated name, plain text still pastes as text, and a
// dropped directory is rejected visibly.
//
// ## What is real here and what is synthesized, stated plainly
//
// PLAN_M4.md's testing decisions require this to be recorded rather than
// glossed, because the honest answer is "most of it, but not the event
// object":
//
// - REAL: the `File` objects (constructed in the page from real bytes),
//   the streamed `fetch` upload, the helm, the supervisor, the file that
//   lands on disk, the WebSocket, and the terminal the path is inserted
//   into. The assertions below read the actual published file off the
//   filesystem and the actual terminal buffer.
// - SYNTHESIZED: the DOM event and its `DataTransfer`/`clipboardData`
//   container. Playwright cannot make an OS-level drag or put a
//   screenshot on the clipboard, and the engines disagree about whether a
//   constructed `ClipboardEvent` even keeps the `clipboardData` it was
//   given — so the event is dispatched at the seam terminal.js listens on,
//   with a stand-in payload object. That is the deterministic path
//   PLAN_M4.md names for image paste, and it is used for drops too rather
//   than having two mechanisms.
// - NOT COVERED HERE: a genuine OS drag and a genuine clipboard image, on
//   the desktop build. Those are the recorded manual pass (PLAN_M4.md
//   acceptance 9), which exists because no browser run can vouch for them.
//
// The directory tests are the one place the synthesis is load-bearing
// rather than incidental: `webkitGetAsEntry` cannot be made to report a
// real directory under synthesis, so both rejection branches are driven
// through the payload stub — the entry-API branch by an item that reports
// `isDirectory`, and the no-entry-API branch by an item whose bytes refuse
// to be read, which is exactly how a directory reaches an engine without
// that API. What they pin is terminal.js's own handling; that a real
// directory produces one of those two shapes is the manual pass's to
// confirm.
// ---------------------------------------------------------------------
import { test, expect, type Page, type APIRequestContext } from "@playwright/test";
import fs from "node:fs";
import { cleanupSession, waitForTermText } from "./helpers/term";
import {
  addTab,
  createTabSession,
  disableReconnectFromNextLoad,
  fulfillAsHelm,
  runInShell,
  selectTerminal,
  sharedSessionRow,
  shellMarker,
  waitForIslandMounted,
  installTerminalSuiteHooks,
} from "./helpers/terminal-suite";

installTerminalSuiteHooks({ tabSweep: true });


/**
 * One entry in a synthesized paste/drop payload.
 *
 * `content` becomes the file's real bytes. `directory` and `unreadable`
 * stand in for the two shapes a dropped folder takes across engines (see
 * this section's header); an entry with either set carries no uploadable
 * bytes by construction, which is the point.
 */
interface PayloadEntry {
  name: string;
  mime: string;
  content?: string;
  directory?: boolean;
  unreadable?: boolean;
  /** Make `DataTransferItem.getAsFile()` throw before any `File` exists. */
  projectionFails?: boolean;
  /**
   * `File.lastModified`, in ms. Left alone (so the engine stamps "now")
   * for the raw-clipboard-data cases; set to something old for the tests
   * that model a real file the user copied, which is half of what tells
   * those two apart (`classify` in farhelm-ui/src/attachments.rs).
   */
  lastModified?: number;
  /**
   * Put this entry only in the payload's `files` list, never in `items`.
   *
   * Some engines expose a payload as a bare `FileList` with no item list
   * at all, and terminal.js has a fallback for exactly that; without a
   * way to synthesize it, the fallback would be reachable only in
   * production.
   */
  filesOnly?: boolean;
}

/**
 * What a synthesized event did, as observed from the page.
 *
 * These are the assertions that separate "the handler ran" from "the
 * handler took responsibility for the event": an interception that
 * forgets `preventDefault` lets the engine navigate to the dropped file,
 * and one that forgets `stopPropagation` lets xterm paste the payload's
 * text on top of the upload.
 */
interface DispatchResult {
  /** Whether anything called `preventDefault()` on the event. */
  defaultPrevented: boolean;
  /** Whether anything called `stopPropagation()` on it. */
  propagationStopped: boolean;
  /**
   * Whether the event reached its own TARGET — xterm's helper textarea
   * for a paste. False means an ancestor's capture handler stopped it on
   * the way down, which is how "intercepted before xterm sees it" is
   * observed rather than assumed.
   */
  reachedTarget: boolean;
  /** For a `dragover`, the drop effect the handler asked for. */
  dropEffect: string;
}

/**
 * Dispatch a synthetic `paste` or `drop` at one island's own element,
 * carrying `entries` as file objects and `text` as `text/plain`, and
 * report what the handlers did with it.
 *
 * A paste is dispatched at xterm's hidden helper TEXTAREA rather than at
 * the mount element, and that is not incidental: xterm registers its own
 * paste handler there, so dispatching anywhere above it would test
 * terminal.js's interception while silently skipping the handler that has
 * to keep working for plain text. Capture-phase interception still sees
 * the event on the way down.
 *
 * A drop is preceded by a `dragover` at the same element, because that is
 * the order a real drag produces and because `drop` does not fire at all
 * on an element whose `dragover` was not prevented — dispatching the drop
 * alone would pass even against an island that is not a drop target.
 *
 * The payload is a stand-in object rather than a real `DataTransfer`
 * because a real one cannot express a directory entry, and mixing two
 * mechanisms would leave the interesting tests on the weaker one.
 */
async function dispatchPayload(
  page: Page,
  elementId: string,
  kind: "paste" | "drop",
  { entries = [], text = "" }: { entries?: PayloadEntry[]; text?: string },
): Promise<{ event: DispatchResult; dragover?: DispatchResult }> {
  return page.evaluate(
    ({ elementId, kind, entries, text }) => {
      const el = document.getElementById(elementId);
      if (!el) throw new Error(`no element ${elementId}`);
      const items: any[] = [];
      const files: any[] = [];
      for (const entry of entries) {
        if (entry.directory) {
          // A directory the drag-entry API reports up front: it has no
          // file behind it at all, which is what the entry API is for.
          items.push({
            kind: "file",
            type: "",
            getAsFile: () => null,
            webkitGetAsEntry: () => ({ isDirectory: true, name: entry.name }),
          });
          continue;
        }
        if (entry.projectionFails) {
          items.push({
            kind: "file",
            type: entry.mime,
            getAsFile: () => { throw new Error("synthetic File projection failure"); },
            webkitGetAsEntry: () => null,
          });
          continue;
        }
        let file: any = new File([entry.content ?? ""], entry.name, {
          type: entry.mime,
          ...(entry.lastModified === undefined ? {} : { lastModified: entry.lastModified }),
        });
        if (entry.unreadable) {
          // A directory on an engine with NO entry API: it arrives looking
          // like a File and only fails when its bytes are read. Modelled
          // by handing back a file-like object whose `slice` yields
          // something a `FileReader` will not read, since a real `File`'s
          // bytes cannot be made to fail on demand.
          //
          // The failure therefore lands on the reader's synchronous throw
          // rather than its `onerror`; a real directory takes the async
          // path, which rejects one line later through the same handler.
          // Both are the probe refusing to vouch for the bytes, which is
          // all the caller acts on.
          file = {
            name: file.name,
            type: file.type,
            size: 4096,
            lastModified: file.lastModified,
            slice: () => ({ notABlob: true }),
          };
        }
        files.push(file);
        if (entry.filesOnly) continue;
        items.push({
          kind: "file",
          type: entry.mime,
          getAsFile: () => file,
          webkitGetAsEntry: () => null,
        });
      }
      const data = {
        items,
        files,
        types: [...(files.length ? ["Files"] : []), ...(text ? ["text/plain"] : [])],
        dropEffect: "none",
        getData: (type: string) => (type === "text/plain" ? text : ""),
      };

      // One dispatch, plus the bookkeeping that makes the handlers'
      // treatment of the event observable from Node.
      const fire = (type: string, target: EventTarget, payloadKey: string | null) => {
        const event = new Event(type, { bubbles: true, cancelable: true });
        if (payloadKey) Object.defineProperty(event, payloadKey, { value: data });
        let propagationStopped = false;
        const realStop = event.stopPropagation.bind(event);
        event.stopPropagation = () => {
          propagationStopped = true;
          realStop();
        };
        let reachedTarget = false;
        const witness = () => {
          reachedTarget = true;
        };
        target.addEventListener(type, witness);
        try {
          target.dispatchEvent(event);
        } finally {
          target.removeEventListener(type, witness);
        }
        return {
          defaultPrevented: event.defaultPrevented,
          propagationStopped,
          reachedTarget,
          dropEffect: data.dropEffect,
        };
      };

      if (kind === "paste") {
        const target = el.querySelector(".xterm-helper-textarea") ?? el;
        return { event: fire("paste", target, "clipboardData") };
      }
      const dragover = fire("dragover", el, "dataTransfer");
      return { event: fire("drop", el, "dataTransfer"), dragover };
    },
    { elementId, kind, entries, text },
  );
}

/**
 * One island's buffer as LOGICAL lines: xterm's wrapped continuation rows
 * are joined back onto the row they continue.
 *
 * The imported `islandText` helper joins every buffer row with a newline,
 * which is right for content assertions and wrong for these: an attachment
 * path is ~70 characters before the session's own directory name, so it
 * wraps in an 80-column terminal and a newline lands in the middle of it.
 * Every assertion here matches a path, so every one of them would fail on a
 * terminal that is behaving perfectly.
 *
 * Rows are translated WITHOUT trailing-space trimming and the assembled
 * line is trimmed at the end instead — trimming each row would silently
 * eat a space that fell on a wrap boundary, which for these tests is the
 * separator between two attached paths.
 */
async function islandLogicalText(page: Page, elementId: string): Promise<string> {
  return page.evaluate((el) => {
    const island = (window as any).__farhelmIslands?.[el];
    if (!island) return "";
    const buf = island.term.buffer.active;
    const lines: string[] = [];
    for (let i = 0; i < buf.length; i++) {
      const line = buf.getLine(i);
      if (!line) continue;
      const text = line.translateToString(false);
      if (line.isWrapped && lines.length) lines[lines.length - 1] += text;
      else lines.push(text);
    }
    return lines.map((line) => line.replace(/\s+$/, "")).join("\n");
  }, elementId);
}

/**
 * A regular-expression fragment matching one inserted host path that ends
 * in `tail` (itself a regex fragment).
 *
 * Anchored at the leading `/`, which is not fussiness: an inserted path
 * arrives through `term.paste()`, so where this client has the pane's
 * bracketed-paste mode it is wrapped in the paste markers, and the pty
 * echoes those control bytes back in caret notation — the buffer really
 * does read `^[[200~/tmp/.../notes.txt ^[[201~`. A pattern starting with
 * `\S*` swallows the `^[[200~` into the path and hands the test a
 * filename that does not exist.
 *
 * The markers are deliberately tolerated rather than required, here and
 * in every `\S*?` that joins two of these fragments: whether THIS client
 * knows the pane is in bracketed-paste mode depends on the tmux underneath
 * (see `latchBracketedPaste`), and no assertion about where a path landed
 * should turn on that. The one test that does pin the markers latches the
 * mode first.
 */
function hostPathEnding(tail: string): string {
  return `\\/\\S*\\/attachments\\/\\S*${tail}`;
}

/**
 * Put this island's xterm into the bracketed-paste mode its pane is
 * already in, so a subsequent paste's markers are observable no matter
 * what the attach snapshot was able to carry.
 *
 * xterm wraps `term.paste()` in `\x1b[200~`/`\x1b[201~` only once it has
 * parsed a `\x1b[?2004h` of its own. The pane genuinely is in that mode —
 * the `basic` fake agent enables it before printing its ready banner — but
 * a client that attaches AFTER the agent said so never sees that byte
 * live: it can only learn the mode from the supervisor's replay, which
 * reconstructs it from tmux's `bracket_paste_flag`. That format variable
 * arrived in tmux 3.7, so against any older tmux the mode is not restored
 * — a degradation the supervisor knows about and warns about once per
 * process — and the paste goes out unwrapped even though the client took
 * exactly the paste path. CI's tmux is one of those, which is why the
 * marker assertion failed there on every run while passing on developer
 * machines with a newer one.
 *
 * So this writes a truth about the pane into the client rather than
 * inventing one, and it leaves the marker assertion pinning what it exists
 * to pin: that insertion went through `term.paste()` rather than straight
 * at the socket, which no amount of restored mode state would fake.
 *
 * Awaited on xterm's own write callback because `term.write` is
 * asynchronous — returning before the parser has run would reintroduce the
 * very race this removes.
 */
async function latchBracketedPaste(page: Page, elementId: string): Promise<void> {
  await page.evaluate(
    (el) =>
      new Promise<void>((resolve, reject) => {
        const island = (window as any).__farhelmIslands?.[el];
        if (!island) {
          reject(new Error(`no island ${el} to put into bracketed-paste mode`));
          return;
        }
        island.term.write("\x1b[?2004h", () => resolve());
      }),
    elementId,
  );
}

/**
 * Poll one island's logical buffer until `pattern` matches, and hand back
 * the match — which is how these tests learn the host path, since the
 * supervisor mints it and nothing client-side can predict it.
 */
async function waitForIslandMatch(
  page: Page,
  elementId: string,
  pattern: RegExp,
  timeout = 30_000,
): Promise<RegExpMatchArray> {
  // A holder rather than a bare `let`: `expect.poll` runs its callback on
  // its own schedule, so TypeScript cannot see that the variable is set
  // by the time the poll resolves.
  const found: { match: RegExpMatchArray | null } = { match: null };
  await expect
    .poll(
      async () => {
        found.match = (await islandLogicalText(page, elementId)).match(pattern);
        return found.match !== null;
      },
      { timeout, message: `waiting for ${pattern} in ${elementId}` },
    )
    .toBe(true);
  return found.match!;
}

/** `waitForIslandText`, over logical (unwrapped) lines. */
async function waitForIslandLogicalText(
  page: Page,
  elementId: string,
  needle: string,
  timeout = 20_000,
) {
  await expect
    .poll(() => islandLogicalText(page, elementId), {
      timeout,
      message: `waiting for ${needle} in ${elementId}`,
    })
    .toContain(needle);
}

/**
 * Count every attachment upload this page issues, so a test can assert
 * that NOTHING was uploaded — the assertion behind "pasted text is still
 * text" and "a directory is rejected", both of which are only interesting
 * if no bytes went anywhere.
 */
function countUploads(page: Page): {
  count: () => number;
  urls: () => string[];
  reset: () => void;
} {
  let seen: string[] = [];
  page.on("request", (req) => {
    if (req.url().includes("/attachments")) seen.push(req.url());
  });
  return {
    count: () => seen.length,
    urls: () => [...seen],
    // For the tests that care about what happened AFTER some point — a
    // remount, say — rather than about the whole page's history.
    reset: () => {
      seen = [];
    },
  };
}

/**
 * Create a session of this section's own and open its terminal, ready to
 * receive a drop.
 *
 * Every attachment test needs clean input and path history. A fresh session
 * starts with an empty input line and has its own banner and prompt, so an
 * insertion assertion cannot match residue left by another test.
 *
 * Reuses the shared suite's `createTabSession` factory: it already mints a
 * per-test working directory and registers it for cleanup, and an
 * attachment test wants exactly that.
 */
async function openAttachmentSession(
  page: Page,
  request: APIRequestContext,
  title: string,
): Promise<{ id: string; cwd: string }> {
  const session = await createTabSession(request, title);
  await page.goto("/");
  await page.locator(`[data-session-id="${session.id}"]`).click();
  await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
  await waitForTermText(page, "FAKE-AGENT READY");
  return session;
}

// The headline flow, with nothing faked but the event container: a file
// dropped on the agent terminal is uploaded to the session's host, lands
// under that session's attachments directory with its bytes intact, and
// its path is inserted AT THE CURSOR — after the text already typed there,
// which is what "at the cursor" has to mean.
//
// Pressing Enter afterwards is not decoration: local echo alone would
// prove the path reached xterm, and the fake agent's `echo:` line is what
// proves those same bytes went out over the WebSocket as terminal input,
// which is the whole point of inserting a path.
test("a file dropped on the agent terminal uploads and inserts its host path at the cursor", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  const name = `notes-${stamp}.txt`;
  const body = `dropped-body-${stamp}`;
  let id: string | undefined;
  try {
    const session = await openAttachmentSession(page, request, `attach-drop-${stamp}`);
    id = session.id;

    await page.locator("#terminal").click();
    await page.keyboard.type("PRE:");
    await dispatchPayload(page, "terminal", "drop", {
      entries: [{ name, mime: "text/plain", content: body }],
      // The text is synthesized alongside the file deliberately: some
      // drag sources do offer a text/plain copy of the path, and the
      // precedence rule exists for exactly that shape. It is not a claim
      // that every real drag carries one — it is how this test reaches
      // the branch where the file has to win.
      text: "/home/somebody/notes.txt",
    });

    const match = await waitForIslandMatch(
      page,
      "terminal",
      new RegExp(`PRE:\\S*?(${hostPathEnding(`${stamp}\\.txt`)})`),
    );
    const hostPath = match[1];
    expect(hostPath, "the path must live under this session's own attachments directory")
      .toContain(`/attachments/${id}/`);
    expect(hostPath.endsWith(name), `dropped files keep their name: ${hostPath}`).toBe(true);
    expect(fs.readFileSync(hostPath, "utf8")).toBe(body);

    await page.keyboard.press("Enter");
    const echoed = `echo:PRE:${hostPath}`;
    await waitForIslandLogicalText(page, "terminal", echoed);
    // One winning interpretation, one insertion: the payload's text/plain
    // copy of the dragged path must not ALSO have been pasted.
    expect(
      await islandLogicalText(page, "terminal"),
      "the file won the payload, so its text/plain sibling must not be pasted too",
    ).not.toContain("/home/somebody/notes.txt");
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// SPEC.md quantifies the attachments contract over "any of a session's
// terminals — the agent's or a tab's", and this is that sentence made
// executable: the drop lands in a TAB, the path arrives in the tab's own
// input, and the agent terminal beside it never sees a thing.
//
// Both halves matter. Per-island hooks that all wrote to the agent
// terminal would pass a test that only checked "a path appeared
// somewhere".
test("a file dropped on a tab inserts its path in that tab, leaving the agent terminal untouched", async ({
  page,
  request,
}) => {
  test.setTimeout(150_000);
  const stamp = Date.now();
  let id: string | undefined;
  try {
    const session = await openAttachmentSession(page, request, `attach-tab-${stamp}`);
    id = session.id;
    const body = `tab-dropped-body-${stamp}`;

    const tabId = await addTab(page, 0);
    const island = `terminal-${tabId}`;
    await waitForIslandMounted(page, island);
    await selectTerminal(page, tabId);

    await dispatchPayload(page, island, "drop", {
      entries: [{ name: "from-tab.txt", mime: "text/plain", content: body }],
    });

    const match = await waitForIslandMatch(
      page,
      island,
      new RegExp(`(${hostPathEnding("from-tab\\.txt")})`),
    );
    const hostPath = match[1];
    expect(hostPath).toContain(`/attachments/${id}/`);
    expect(fs.readFileSync(hostPath, "utf8")).toBe(body);

    expect(
      await islandLogicalText(page, "terminal"),
      "the agent terminal received no drop, so nothing may have been inserted into it",
    ).not.toContain("/attachments/");
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// A pasted screenshot: image data on the clipboard is uploaded under a
// GENERATED name rather than the placeholder the engine invented for it
// (Chromium calls every pasted image `image.png`), which is why the
// payload below carries exactly that placeholder — using a nameless entry
// would test a case real engines do not produce.
//
// The extension comes from the MIME type, so the agent reading the path
// knows what it is holding.
test("a pasted image is uploaded under a generated name and its path inserted", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  const body = `pasted-image-bytes-${stamp}`;
  let id: string | undefined;
  try {
    const session = await openAttachmentSession(page, request, `attach-image-${stamp}`);
    id = session.id;

    await dispatchPayload(page, "terminal", "paste", {
      entries: [{ name: "image.png", mime: "image/png", content: body }],
      // The other half of the precedence order: a clipboard holding image
      // data very often holds a text rendering beside it (an HTML wrapper,
      // a source URL). The image has to win and the text must not also be
      // pasted, which is the "one winning interpretation, one insertion"
      // rule seen from the image side.
      text: "https://example.invalid/screenshot.png",
    });

    const match = await waitForIslandMatch(
      page,
      "terminal",
      new RegExp(`(${hostPathEnding("pasted-\\d+(?:-\\d+)?\\.png")})`),
    );
    const hostPath = match[1];
    expect(hostPath).toContain(`/attachments/${id}/`);
    // The counter is per page and this is the page's first paste, so the
    // name is `pasted-1.png`; the optional numeric suffix is the
    // supervisor's collision resolution, which nothing here should hit but
    // which is not worth failing over if the directory is ever reused.
    expect(
      /\/pasted-1(-\d+)?\.png$/.test(hostPath),
      `a pasted image takes a generated, typed name, got ${hostPath}`,
    ).toBe(true);
    expect(fs.readFileSync(hostPath, "utf8")).toBe(body);
    expect(
      await islandLogicalText(page, "terminal"),
      "the image won the payload, so its text sibling must not be pasted too",
    ).not.toContain("example.invalid");
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// SPEC.md is explicit that pasted text which merely LOOKS like a path is
// still text, and the way to keep that true is to have no path heuristic
// at all — the classifier looks at what the payload carries, never at what
// the text says. This pins both halves: the text arrives as terminal
// input, and no upload was attempted.
//
// It also exercises xterm's own paste handler, which an interception bug
// could break by swallowing every paste event indiscriminately.
test("pasted text that looks like a path arrives as text, with nothing uploaded", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  const uploads = countUploads(page);
  let id: string | undefined;
  try {
    const session = await openAttachmentSession(page, request, `attach-text-${stamp}`);
    id = session.id;

    await dispatchPayload(page, "terminal", "paste", { text: "/etc/hosts" });

    await waitForIslandMatch(page, "terminal", /\/etc\/hosts/);
    // Focus the terminal before Enter: the auto-selected load leaves the
    // caret wherever the last click landed, and a synthetic paste dispatch
    // does not move it.
    await page.locator("#terminal").click();
    await page.keyboard.press("Enter");
    await waitForIslandLogicalText(page, "terminal", "echo:/etc/hosts");
    expect(uploads.count(), "text is not an attachment").toBe(0);
    await expect(page.locator('[data-terminal="agent"] .attach-error')).toHaveCount(0);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// SPEC.md rejects dropped directories unconditionally in v1, and the
// drag-entry API is how that is caught before anything is read. The
// payload is synthesized with a `text/plain` copy of the folder's path
// beside it — a shape folder drags do produce on some sources — because
// the interesting failure is silently pasting THAT instead: a directory
// outranks text in the precedence order precisely so the rejection is
// what the user gets.
test("a dropped directory is rejected visibly, uploading nothing and pasting nothing", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  const uploads = countUploads(page);
  let id: string | undefined;
  try {
    const session = await openAttachmentSession(page, request, `attach-dir-${stamp}`);
    id = session.id;

    await dispatchPayload(page, "terminal", "drop", {
      entries: [{ name: "some-folder", mime: "", directory: true }],
      text: "/home/somebody/some-folder",
    });

    const error = page.locator('[data-terminal="agent"] .attach-error');
    await expect(error).toBeVisible();
    await expect(error).toContainText("some-folder");
    await expect(error).toContainText("is a directory");
    expect(uploads.count(), "nothing directory-shaped may reach the helm").toBe(0);
    expect(
      await islandLogicalText(page, "terminal"),
      "the folder's own path must not be pasted as a consolation prize",
    ).not.toContain("/home/somebody/some-folder");
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The other half of the unconditional rejection: an engine with no
// drag-entry API hands a directory over looking like an ordinary `File`,
// and the only thing that gives it away is that its bytes cannot be read.
// terminal.js reads one byte before uploading for exactly this reason, so
// the refusal happens BEFORE anything is sent rather than arriving as a
// truncated upload the supervisor rejects for its size.
//
// The wording deliberately does not claim to know it was a directory — an
// unreadable file reaches this same path — but it does name what it saw.
test("an item whose bytes cannot be read is rejected before anything is uploaded", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  const uploads = countUploads(page);
  let id: string | undefined;
  try {
    const session = await openAttachmentSession(page, request, `attach-unreadable-${stamp}`);
    id = session.id;

    await dispatchPayload(page, "terminal", "drop", {
      entries: [{ name: "looks-like-a-file", mime: "", unreadable: true }],
    });

    const error = page.locator('[data-terminal="agent"] .attach-error');
    await expect(error).toBeVisible();
    await expect(error).toContainText("looks-like-a-file");
    await expect(error).toContainText("could not be read");
    expect(uploads.count(), "the probe read must fail before the upload starts").toBe(0);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// SPEC.md: "Upload failures must be visible; an attachment must never
// disappear silently." A failure is induced at the network seam because a
// healthy stack will not produce one on demand, and both obligations are
// checked — the user is told, in the supervisor's own words, and no path
// is inserted for a file that was never published.
// An upload is the ONE request this UI makes that does not go through the
// Rust HTTP funnel where every other reply's build stamp is read: a `File`
// cannot cross that boundary, so it goes out as a `fetch` from
// terminal.js. That makes it the one path that could observe nothing, and
// the one place the check has to be written twice.
//
// Both shapes of disagreement are covered, because they reach the code
// differently and say different things: a helm reporting a DIFFERENT build
// names it, and a helm reporting NONE is a mismatch too (a conforming helm
// both sends the header and exposes it to this cross-origin read, so
// absence means the peer is not this build). Asserted on a REFUSED upload
// as well as a successful one, since a stale bundle's failures are at
// least as likely to arrive as inexplicable refusals.
for (const stamp of [
  { name: "a different build", header: "9999.0.0-from-a-newer-helm", says: "9999.0.0" },
  { name: "no build at all", header: null, says: "no build at all" },
]) {
  test(`upload-skew-surfaces-on-the-terminal (${stamp.name})`, async ({ page, request }) => {
    test.setTimeout(120_000);
    const landed = `/tmp/fh-skew-${Date.now()}/attachments/s/note.txt`;
    let id: string | undefined;
    try {
      // The ONE fixture in this file that deliberately does not use
      // `fulfillAsHelm`: the wrong stamp (or its absence) IS the subject
      // here, so it is written out rather than borrowed from the helper
      // whose whole job is to get it right.
      await page.route("**/api/sessions/*/attachments*", async (route) => {
        const headers: Record<string, string> = {
          "content-type": "application/json",
        };
        if (stamp.header) headers["x-farhelm-build"] = stamp.header;
        await route.fulfill({
          status: 200,
          headers,
          body: JSON.stringify({ path: landed }),
        });
      });
      const session = await openAttachmentSession(page, request, `attach-skew-${Date.now()}`);
      id = session.id;

      await dispatchPayload(page, "terminal", "drop", {
        entries: [{ name: "note.txt", mime: "text/plain", content: "hello" }],
      });

      const error = page.locator('[data-terminal="agent"] .attach-error');
      await expect(error).toBeVisible({ timeout: 20_000 });
      await expect(error).toContainText(stamp.says);
      await expect(
        error,
        "the remedy belongs on the line the user is already reading",
      ).toContainText("reload");
    } finally {
      if (id) await cleanupSession(request, id);
    }
  });
}

test("an upload that fails surfaces the error and inserts no path", async ({ page, request }) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  let id: string | undefined;
  try {
    await page.route("**/api/sessions/*/attachments*", async (route) => {
      await fulfillAsHelm(route, {
        status: 500,
        contentType: "text/plain",
        body: "storing the attachment failed: no space left on device\n",
      });
    });
    const session = await openAttachmentSession(page, request, `attach-fail-${stamp}`);
    id = session.id;

    await dispatchPayload(page, "terminal", "drop", {
      entries: [{ name: "doomed.txt", mime: "text/plain", content: "never lands" }],
    });

    const error = page.locator('[data-terminal="agent"] .attach-error');
    await expect(error).toBeVisible();
    await expect(error).toContainText("doomed.txt");
    await expect(
      error,
      "the response body — here injected, in production the supervisor's own words — is what \
makes the error actionable, so it has to reach the line the user reads",
    ).toContainText("no space left on device");
    await expect(error).toContainText("no path was inserted");
    expect(
      await islandLogicalText(page, "terminal"),
      "a failed upload published nothing, so there is no path to insert",
    ).not.toContain("/attachments/");
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// SPEC.md: "Transfer must not block the terminal: you can keep typing
// while it runs, and the path is inserted at whatever cursor position is
// current when the transfer completes." Both clauses are one assertion
// here — the characters typed DURING the upload land first, and the path
// lands after them.
//
// The upload is throttled at the network seam rather than by uploading
// something genuinely large: what this test needs is a transfer that is
// still running while a human types, and a delayed `continue()` gives that
// deterministically while leaving the upload itself entirely real.
test("typing stays live during an upload, and the path lands at the cursor position it finds", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  const name = `slow-${stamp}.txt`;
  const body = `slow-upload-${stamp}`;
  let id: string | undefined;
  try {
    await page.route("**/api/sessions/*/attachments*", async (route) => {
      await new Promise((resolve) => setTimeout(resolve, 3_000));
      await route.continue();
    });
    const session = await openAttachmentSession(page, request, `attach-slow-${stamp}`);
    id = session.id;

    await page.locator("#terminal").click();
    await page.keyboard.type("PRE:");
    await dispatchPayload(page, "terminal", "drop", {
      entries: [{ name, mime: "text/plain", content: body }],
    });

    // The indicator is up while the transfer runs, and it names the file.
    const busy = page.locator('[data-terminal="agent"] .attach-busy');
    await expect(busy).toBeVisible();
    await expect(busy).toContainText(name);

    // Typed while the upload is still in flight: if the terminal were
    // blocked, these keystrokes would not be echoed until afterwards, and
    // the ordering assertion below would fail.
    await page.keyboard.type("LIVE");
    await waitForIslandMatch(page, "terminal", /PRE:LIVE/);

    const match = await waitForIslandMatch(
      page,
      "terminal",
      new RegExp(`PRE:LIVE\\S*?(${hostPathEnding(`${stamp}\\.txt`)})`),
    );
    expect(match[1]).toContain(`/attachments/${id}/`);
    expect(fs.readFileSync(match[1], "utf8")).toBe(body);
    // The indicator goes away on its own once nothing is in flight.
    await expect(busy).toHaveCount(0);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// "N dropped files insert N paths" (PLAN_M4.md acceptance 6), separated so
// a shell sees N words rather than one impossible one. The separator is a
// trailing space per path (`PATH_SEPARATOR` in
// farhelm-ui/src/attachments.rs), which is what also keeps a SECOND drop
// from fusing onto the end of the first one's path.
test("two dropped files insert both paths, each its own word", async ({ page, request }) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  let id: string | undefined;
  try {
    const session = await openAttachmentSession(page, request, `attach-many-${stamp}`);
    id = session.id;

    await dispatchPayload(page, "terminal", "drop", {
      entries: [
        { name: `first-${stamp}.txt`, mime: "text/plain", content: "first body" },
        { name: `second-${stamp}.txt`, mime: "text/plain", content: "second body" },
      ],
    });

    // Each path is its own `term.paste()`, so each carries its own
    // bracketed-paste wrapper and the echoed markers land BETWEEN them:
    // `^[[200~<first> ^[[201~^[[200~<second> ^[[201~`. The space after the
    // first path is the separator under test; the `\S*?` after it is the
    // marker noise the pty echoed (see `hostPathEnding`).
    const match = await waitForIslandMatch(
      page,
      "terminal",
      new RegExp(
        `(${hostPathEnding(`first-${stamp}\\.txt`)}) \\S*?(${
          hostPathEnding(`second-${stamp}\\.txt`)
        })`,
      ),
    );
    for (const [hostPath, expected] of [[match[1], "first body"], [match[2], "second body"]]) {
      expect(hostPath).toContain(`/attachments/${id}/`);
      expect(fs.readFileSync(hostPath, "utf8")).toBe(expected);
    }
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// Two REAL files in one clipboard payload, one of them an image: both are
// file references, so both upload and both keep their own names.
//
// The regression this pins is a silent one. An earlier rule sorted every
// image-typed clipboard entry into the image bucket, so a payload holding
// a document and a picture classified as "file" and then uploaded only the
// document — the picture vanished with no error anywhere, which is exactly
// the silent loss SPEC.md forbids. The image entry here carries a real
// name and an old `lastModified`, which is what a copied FILE looks like
// (see `classify` in farhelm-ui/src/attachments.rs).
test("a clipboard payload of two real files uploads both, each under its own name", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  let id: string | undefined;
  try {
    const session = await openAttachmentSession(page, request, `attach-two-real-${stamp}`);
    id = session.id;

    await dispatchPayload(page, "terminal", "paste", {
      entries: [
        {
          name: `notes-${stamp}.txt`,
          mime: "text/plain",
          content: "document body",
          lastModified: stamp - 14 * 24 * 60 * 60 * 1000,
        },
        {
          name: `holiday-${stamp}.png`,
          mime: "image/png",
          content: "picture body",
          // A week old: a file the user copied, not bytes the engine
          // synthesized a moment ago.
          lastModified: stamp - 7 * 24 * 60 * 60 * 1000,
        },
      ],
    });

    const match = await waitForIslandMatch(
      page,
      "terminal",
      new RegExp(
        `(${hostPathEnding(`notes-${stamp}\\.txt`)}) \\S*?(${
          hostPathEnding(`holiday-${stamp}\\.png`)
        })`,
      ),
    );
    for (const [hostPath, expected] of [[match[1], "document body"], [match[2], "picture body"]]) {
      expect(hostPath).toContain(`/attachments/${id}/`);
      expect(fs.readFileSync(hostPath, "utf8")).toBe(expected);
    }
    const facts = page.locator(".clipboard-facts");
    await expect(facts).toBeVisible();
    await facts.evaluate((details: HTMLDetailsElement) => { details.open = true; });
    const captured = JSON.parse((await facts.locator("pre").textContent()) || "null");
    expect(
      captured.items.map(({ order, kind, type, fileName }: any) => ({
        order,
        kind,
        type,
        fileName,
      })),
    ).toEqual([
      { order: 0, kind: "file", type: "text/plain", fileName: `notes-${stamp}.txt` },
      { order: 1, kind: "file", type: "image/png", fileName: `holiday-${stamp}.png` },
    ]);
    expect(
      captured.files.map(({ order, kind, type, fileName }: any) => ({
        order,
        kind,
        type,
        fileName,
      })),
    ).toEqual([
      { order: 0, kind: "file", type: "text/plain", fileName: `notes-${stamp}.txt` },
      { order: 1, kind: "file", type: "image/png", fileName: `holiday-${stamp}.png` },
    ]);
    expect(
      await islandLogicalText(page, "terminal"),
      "a copied image FILE keeps its own name; only raw clipboard data is renamed",
    ).not.toContain("pasted-");
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// A Mac-only failure can expose a file item while refusing its `File` object.
// The event remains xterm's business, but its serializable evidence must stay
// on screen so the manual run can carry the exact item shape into a fixture.
test("clipboard facts survive a failed File projection without intercepting paste", async ({
  page,
  request,
}) => {
  let id: string | undefined;
  try {
    const session = await openAttachmentSession(page, request, `attach-facts-failure-${Date.now()}`);
    id = session.id;
    const dispatched = await dispatchPayload(page, "terminal", "paste", {
      entries: [{
        name: "unavailable.tiff",
        mime: "image/tiff",
        projectionFails: true,
      }],
    });
    expect(dispatched.event.defaultPrevented).toBe(false);
    expect(
      dispatched.event.reachedTarget,
      "the diagnostic observer must not stop the unsupported paste before xterm sees it",
    ).toBe(true);

    const facts = page.locator(".clipboard-facts");
    await expect(facts).toBeVisible();
    await facts.evaluate((details: HTMLDetailsElement) => { details.open = true; });
    const captured = JSON.parse((await facts.locator("pre").textContent()) || "null");
    expect(captured.items).toEqual([{
      order: 0,
      kind: "file",
      type: "image/tiff",
      fileName: null,
      fileType: null,
      lastModified: null,
    }]);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The precedence case with all three flavors present at once: a file
// reference, raw image DATA, and text.
//
// One upload, one insertion, and the file's own name — because the image
// data and the file are two REPRESENTATIONS of one copy (copying an image
// file offers both), so uploading both would publish the same picture
// twice under two names. That is what the precedence order is for, and it
// is a different question from the test above, where two distinct files
// both had to survive.
test("a file reference beside raw image data uploads the file alone", async ({ page, request }) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  const uploads = countUploads(page);
  let id: string | undefined;
  try {
    const session = await openAttachmentSession(page, request, `attach-mixed-${stamp}`);
    id = session.id;

    await dispatchPayload(page, "terminal", "paste", {
      entries: [
        {
          name: `report-${stamp}.txt`,
          mime: "text/plain",
          content: "the file itself",
          lastModified: stamp - 60_000,
        },
        // No name at all: unambiguously the engine's rendering of the
        // clipboard's bytes rather than a file the user chose.
        { name: "", mime: "image/png", content: "the rendering" },
      ],
      text: `/home/somebody/report-${stamp}.txt`,
    });

    const match = await waitForIslandMatch(
      page,
      "terminal",
      new RegExp(`(${hostPathEnding(`report-${stamp}\\.txt`)})`),
    );
    expect(fs.readFileSync(match[1], "utf8")).toBe("the file itself");
    const buffer = await islandLogicalText(page, "terminal");
    expect(buffer, "the rendering is not a second attachment").not.toContain("pasted-");
    expect(buffer, "and the text sibling is not pasted either").not.toContain("/home/somebody/");
    expect(uploads.count(), "one copy is one upload").toBe(1);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The inserted path is TERMINAL INPUT, so a path a shell would split is a
// path the agent cannot open. The supervisor sanitizes the filename, but
// the directories above it come from the user's own `--state-dir`, which
// can be `~/Library/Application Support/…` or anything else a filesystem
// allows.
//
// Driven in a TAB, against a real shell, because that is the only way to
// assert the property that matters: not "the text looks quoted" but "the
// shell sees ONE argument". The published path is injected rather than
// produced, since the suite's own state directory is a plain mktemp name —
// making the stack use a hostile one would mean restarting it for a single
// test.
test("a path whose directories need quoting arrives at the shell as one word", async ({
  page,
  request,
}) => {
  test.setTimeout(150_000);
  const stamp = Date.now();
  const hostile = `/tmp/fh attach '${stamp}'/attachments/s/shot.png`;
  let id: string | undefined;
  try {
    await page.route("**/api/sessions/*/attachments*", async (route) => {
      await fulfillAsHelm(route, {
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ path: hostile }),
      });
    });
    const session = await openAttachmentSession(page, request, `attach-quote-${stamp}`);
    id = session.id;

    const tabId = await addTab(page, 0);
    const island = `terminal-${tabId}`;
    await waitForIslandMounted(page, island);
    await selectTerminal(page, tabId);

    // `sh -c` for the same portability reason `shellMarker` uses it: the
    // tab runs the user's real login shell, which may be fish. `$1` is
    // printed whole, so a path the outer shell split would show up
    // truncated at the first space.
    await runInShell(page, island, `sh -c 'echo READY-${stamp}'`, `READY-${stamp}`);
    await page.locator(`[id="${island}"]`).click();
    await page.keyboard.type(`sh -c 'echo ARG:"$1"' _ `);
    await dispatchPayload(page, island, "drop", {
      entries: [{ name: "shot.png", mime: "image/png", content: "x" }],
    });
    await waitForIslandMatch(page, island, /shot\.png/);
    await page.keyboard.press("Enter");

    await waitForIslandLogicalText(page, island, `ARG:${hostile}`, 30_000);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// SPEC.md's "an attachment must never disappear silently", at the one seam
// where it is easiest to break: `term.paste()` into an island whose socket
// is gone drops the text with no error anywhere. A payload offered to a
// detached terminal is therefore refused up front, visibly, and nothing is
// uploaded for it.
//
// The socket is closed directly rather than by staging a real takeover:
// what this exercises is the liveness gate, and a takeover would add a
// second client's timing to a test that has nothing to say about it.
test("a payload dropped on a detached terminal is refused instead of uploaded", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  const uploads = countUploads(page);
  let id: string | undefined;
  try {
    // The terminal has to STAY detached for the drop below to land on the
    // state under test; auto-reconnect would otherwise have it back in
    // under a second (see `disableReconnectFromNextLoad`).
    await disableReconnectFromNextLoad(page);
    const session = await openAttachmentSession(page, request, `attach-detached-${stamp}`);
    id = session.id;

    await page.evaluate(() => (window as any).__farhelmIslands["terminal"].ws.close());
    await page.waitForFunction(
      () => (window as any).__farhelmIslands["terminal"].ws.readyState !== WebSocket.OPEN,
    );

    await dispatchPayload(page, "terminal", "drop", {
      entries: [{ name: "late.txt", mime: "text/plain", content: "nowhere to go" }],
    });

    const error = page.locator('[data-terminal="agent"] .attach-error');
    await expect(error).toBeVisible();
    await expect(error).toContainText("not connected");
    expect(uploads.count(), "a path with nowhere to land is not worth the transfer").toBe(0);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The other side of the same seam: the terminal was live when the upload
// started and gone by the time it finished. The file is real and published
// by then, so the honest answer is to hand the user its path rather than
// paste it into a socket that will drop it.
test("an upload that outlives its socket reports the path it could not insert", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  const landed = `/tmp/fh-landed-${stamp}/attachments/s/late.png`;
  let id: string | undefined;
  try {
    await page.route("**/api/sessions/*/attachments*", async (route) => {
      await new Promise((resolve) => setTimeout(resolve, 2_000));
      await fulfillAsHelm(route, {
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ path: landed }),
      });
    });
    // Same reason as the test above: the upload has to complete into a
    // terminal that is still gone.
    await disableReconnectFromNextLoad(page);
    const session = await openAttachmentSession(page, request, `attach-outlives-${stamp}`);
    id = session.id;

    await dispatchPayload(page, "terminal", "drop", {
      entries: [{ name: "late.png", mime: "image/png", content: "bytes" }],
    });
    await expect(page.locator('[data-terminal="agent"] .attach-busy')).toBeVisible();
    // Killed while the upload is in flight, so completion lands on a
    // terminal that can no longer receive anything.
    await page.evaluate(() => (window as any).__farhelmIslands["terminal"].ws.close());

    const error = page.locator('[data-terminal="agent"] .attach-error');
    await expect(error).toBeVisible({ timeout: 20_000 });
    await expect(error).toContainText(landed);
    await expect(error).toContainText("not inserted");
    expect(
      await islandLogicalText(page, "terminal"),
      "the path was reported, not pasted into a socket that would swallow it",
    ).not.toContain(landed);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The failure message is assembled from a template and values that include
// the server's own words, and both halves of that assembly have bitten
// before.
//
// A string replacement would let `$&` and `$1` in the server's text
// rewrite the message around them, and a per-key sequence of replacements
// would re-scan what an earlier key inserted — so a file named
// `{reason}.txt` would have the error spliced into its own name. Both are
// asserted EXACTLY here, so a revert to either shape fails rather than
// producing something merely odd-looking.
test("error text carrying replacement tokens renders literally", async ({ page, request }) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  const hostileReason = "storage failed: $& and $1 and $` are literal";
  let id: string | undefined;
  try {
    await page.route("**/api/sessions/*/attachments*", async (route) => {
      await fulfillAsHelm(route, { status: 500, contentType: "text/plain", body: hostileReason });
    });
    const session = await openAttachmentSession(page, request, `attach-tokens-${stamp}`);
    id = session.id;

    await dispatchPayload(page, "terminal", "drop", {
      entries: [{ name: "{reason}.txt", mime: "text/plain", content: "x" }],
    });

    const error = page.locator('[data-terminal="agent"] .attach-error');
    await expect(error).toBeVisible();
    expect(await error.textContent()).toBe(
      `attaching {reason}.txt failed: ${hostileReason} — no path was inserted`,
    );
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The indicator has to describe what is STILL RUNNING, which a count plus
// a remembered name cannot do: finish the second of two uploads and the
// count says one while the remembered name is the one that just landed, so
// the line names the wrong file for as long as the other runs.
//
// The two uploads are made to finish out of order deliberately — the
// second returns immediately, the first is held — which is the exact
// arrangement that exposed it.
test("the indicator names the upload still running, not the one that just finished", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  let id: string | undefined;
  try {
    await page.route("**/api/sessions/*/attachments*", async (route) => {
      // The quick one is not instant: both have to be in flight together
      // long enough for the many-files form to be observable, or this
      // test would pass without ever seeing the state it is about.
      const slow = route.request().url().includes("slow");
      await new Promise((resolve) => setTimeout(resolve, slow ? 6_000 : 1_500));
      await fulfillAsHelm(route, {
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ path: `/tmp/fh-${stamp}/attachments/s/${slow ? "slow" : "quick"}` }),
      });
    });
    const session = await openAttachmentSession(page, request, `attach-order-${stamp}`);
    id = session.id;

    // Two payloads rather than one drop of two files: uploads within one
    // payload are sequential by design, so overlapping them takes two.
    await dispatchPayload(page, "terminal", "drop", {
      entries: [{ name: "slow.txt", mime: "text/plain", content: "1" }],
    });
    await dispatchPayload(page, "terminal", "drop", {
      entries: [{ name: "quick.txt", mime: "text/plain", content: "2" }],
    });

    const busy = page.locator('[data-terminal="agent"] .attach-busy');
    await expect(busy).toContainText("2 files");
    // The quick one lands first; the line must then name the slow one.
    await expect(busy).toContainText("slow.txt", { timeout: 15_000 });
    await expect(busy).toHaveCount(0, { timeout: 20_000 });
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// An error stays on screen until the user does something that supersedes
// it. A drag carrying nothing this UI can act on is not that: the default
// is still prevented (an unhandled drop navigates the page away, taking
// every terminal with it), but the failure the user has not read yet stays
// where it was.
test("an unsupported drop leaves an earlier failure on screen", async ({ page, request }) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  let id: string | undefined;
  try {
    const session = await openAttachmentSession(page, request, `attach-none-${stamp}`);
    id = session.id;

    await dispatchPayload(page, "terminal", "drop", {
      entries: [{ name: "some-folder", mime: "", directory: true }],
    });
    const error = page.locator('[data-terminal="agent"] .attach-error');
    await expect(error).toContainText("some-folder");

    const empty = await dispatchPayload(page, "terminal", "drop", {});
    expect(
      empty.event.defaultPrevented,
      "an unhandled drop is the engine's to act on, and its action is to navigate away",
    ).toBe(true);
    await expect(error).toContainText("some-folder");
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// A zero-byte file is an ordinary attachment: the pinned contract has no
// minimum size, and the readability probe — which reads one byte — must
// not mistake "there is no first byte" for "these bytes cannot be read".
test("a zero-byte file publishes and inserts its path", async ({ page, request }) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  let id: string | undefined;
  try {
    const session = await openAttachmentSession(page, request, `attach-empty-${stamp}`);
    id = session.id;

    await dispatchPayload(page, "terminal", "drop", {
      entries: [{ name: `empty-${stamp}.txt`, mime: "text/plain", content: "" }],
    });

    const match = await waitForIslandMatch(
      page,
      "terminal",
      new RegExp(`(${hostPathEnding(`empty-${stamp}\\.txt`)})`),
    );
    expect(fs.readFileSync(match[1], "utf8")).toBe("");
    await expect(page.locator('[data-terminal="agent"] .attach-error')).toHaveCount(0);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The proposed name goes into a URL QUERY, so a filename carrying `&`,
// `#`, `?` or a space must not be able to add parameters, truncate the
// URL, or split it. Both ends are checked: the request leaves
// percent-encoded, and the file publishes with its bytes intact under the
// supervisor's own sanitized spelling of the name.
test("a filename full of URL syntax reaches the helm percent-encoded", async ({ page, request }) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  const uploads = countUploads(page);
  const hostile = `a&b#c?d e-${stamp}.txt`;
  let id: string | undefined;
  try {
    const session = await openAttachmentSession(page, request, `attach-urlname-${stamp}`);
    id = session.id;

    await dispatchPayload(page, "terminal", "drop", {
      entries: [{ name: hostile, mime: "text/plain", content: "encoded body" }],
    });

    const match = await waitForIslandMatch(
      page,
      "terminal",
      new RegExp(`(${hostPathEnding(`${stamp}.txt`)})`),
    );
    expect(fs.readFileSync(match[1], "utf8")).toBe("encoded body");
    const [url] = uploads.urls();
    expect(url).toContain(`filename=a%26b%23c%3Fd%20e-${stamp}.txt`);
    expect(
      new URL(url).searchParams.get("filename"),
      "every byte of the name must survive as ONE query value",
    ).toBe(hostile);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// Two ways an upload can fail without the server ever saying so: the
// request never completes (a dropped connection), and it completes with a
// body that is not the contract's shape. Both have to surface visibly and
// insert nothing — a 200 with no usable path is not a success with
// nothing to show, it is a failure this client cannot explain further.
test("a dead request and a malformed reply both fail visibly", async ({ page, request }) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  let mode: "abort" | "garbage" = "abort";
  let id: string | undefined;
  try {
    await page.route("**/api/sessions/*/attachments*", async (route) => {
      if (mode === "abort") {
        await route.abort("connectionreset");
        return;
      }
      await fulfillAsHelm(route, { status: 200, contentType: "application/json", body: "{\"ok\":true}" });
    });
    const session = await openAttachmentSession(page, request, `attach-broken-${stamp}`);
    id = session.id;

    await dispatchPayload(page, "terminal", "drop", {
      entries: [{ name: "dropped.txt", mime: "text/plain", content: "x" }],
    });
    const error = page.locator('[data-terminal="agent"] .attach-error');
    await expect(error).toBeVisible();
    await expect(error).toContainText("dropped.txt");
    await expect(error).toContainText("no path was inserted");

    mode = "garbage";
    await dispatchPayload(page, "terminal", "drop", {
      entries: [{ name: "shapeless.txt", mime: "text/plain", content: "x" }],
    });
    await expect(error).toContainText("shapeless.txt");
    await expect(error).toContainText("no usable path");
    expect(
      await islandLogicalText(page, "terminal"),
      "neither failure published anything, so neither may insert anything",
    ).not.toContain("/attachments/");
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// One bad file in a drop of several must not cost the user the rest. The
// first upload is refused and the second is left alone, so the queue has
// to report the failure and carry on rather than abandoning what follows.
test("a failure on the first of two dropped files does not stop the second", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  let id: string | undefined;
  try {
    await page.route("**/api/sessions/*/attachments*", async (route) => {
      if (route.request().url().includes("doomed")) {
        await fulfillAsHelm(route, { status: 500, contentType: "text/plain", body: "refused" });
        return;
      }
      await route.continue();
    });
    const session = await openAttachmentSession(page, request, `attach-partial-${stamp}`);
    id = session.id;

    await dispatchPayload(page, "terminal", "drop", {
      entries: [
        { name: `doomed-${stamp}.txt`, mime: "text/plain", content: "never" },
        { name: `survivor-${stamp}.txt`, mime: "text/plain", content: "landed anyway" },
      ],
    });

    const match = await waitForIslandMatch(
      page,
      "terminal",
      new RegExp(`(${hostPathEnding(`survivor-${stamp}\\.txt`)})`),
    );
    expect(fs.readFileSync(match[1], "utf8")).toBe("landed anyway");
    const error = page.locator('[data-terminal="agent"] .attach-error');
    await expect(error).toContainText(`doomed-${stamp}.txt`);
    expect(
      await islandLogicalText(page, "terminal"),
      "the refused file published nothing, so no path for it may appear",
    ).not.toContain("doomed-");
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// Leaving a session mid-upload tears the island down, and the remount that
// follows must be a clean one. The listeners live on an element Dioxus
// keeps, so a teardown that forgot to remove them would stack a second set
// on the remounted island — and the next drop would upload the same file
// twice and insert its path twice.
test("leaving mid-upload and reopening leaves exactly one set of hooks", async ({
  page,
  request,
}) => {
  test.setTimeout(150_000);
  const stamp = Date.now();
  const uploads = countUploads(page);
  let id: string | undefined;
  try {
    await page.route("**/api/sessions/*/attachments*", async (route) => {
      if (route.request().url().includes("abandoned")) {
        await new Promise((resolve) => setTimeout(resolve, 30_000));
      }
      await route.continue();
    });
    const session = await openAttachmentSession(page, request, `attach-remount-${stamp}`);
    id = session.id;

    await dispatchPayload(page, "terminal", "drop", {
      entries: [{ name: `abandoned-${stamp}.txt`, mime: "text/plain", content: "x" }],
    });
    await expect(page.locator('[data-terminal="agent"] .attach-busy')).toBeVisible();

    await sharedSessionRow(page).click();
    await expect(page.locator(".titlebar .title")).toHaveText("e2e-session");
    await page.locator(`[data-session-id="${id}"]`).click();
    await page.waitForFunction(() => (window as any).__farhelmTermReady === true);
    await waitForTermText(page, "FAKE-AGENT READY");

    uploads.reset();
    await dispatchPayload(page, "terminal", "drop", {
      entries: [{ name: `after-${stamp}.txt`, mime: "text/plain", content: "once" }],
    });
    const match = await waitForIslandMatch(
      page,
      "terminal",
      new RegExp(`(${hostPathEnding(`after-${stamp}\\.txt`)})`),
    );
    expect(fs.readFileSync(match[1], "utf8")).toBe("once");
    expect(uploads.count(), "one drop is one upload, even after a remount").toBe(1);
    const buffer = await islandLogicalText(page, "terminal");
    expect(
      buffer.split(`after-${stamp}.txt`).length - 1,
      "and one upload is one insertion",
    ).toBe(1);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The status line carries two strings this page did not author: a filename
// from the user's filesystem and an error body from a supervisor that
// under `--ssh` is another machine. Both are rendered as TEXT — the same
// promise the session list makes for titles and error details, checked the
// same way: the markup survives as characters and creates no element.
test("a filename and a server error carrying markup render as literal text", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  const markupName = `<img src=x onerror="window.__pwned=1">-${stamp}.txt`;
  let id: string | undefined;
  try {
    await page.route("**/api/sessions/*/attachments*", async (route) => {
      await fulfillAsHelm(route, {
        status: 500,
        contentType: "text/plain",
        body: '<script>window.__pwned = 2</script>refused',
      });
    });
    const session = await openAttachmentSession(page, request, `attach-xss-${stamp}`);
    id = session.id;

    await dispatchPayload(page, "terminal", "drop", {
      entries: [{ name: markupName, mime: "text/plain", content: "x" }],
    });

    const error = page.locator('[data-terminal="agent"] .attach-error');
    await expect(error).toBeVisible();
    const rendered = await error.textContent();
    expect(rendered).toContain(markupName);
    expect(rendered).toContain("<script>window.__pwned = 2</script>");
    expect(
      await error.locator("img, script").count(),
      "markup in a name or a server message is content, never structure",
    ).toBe(0);
    expect(await page.evaluate(() => (window as any).__pwned)).toBeUndefined();
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// Two properties of the moment an upload completes, both invisible until
// they break.
//
// The path must not steal FOCUS: an upload finishes whenever it finishes,
// and yanking the caret out of whatever the user moved to in the meantime
// would be worse than the wait. And it must arrive through the
// bracketed-paste path, which is what tells a full-screen agent that the
// text was pasted rather than typed — visible here as the markers the pty
// echoes back around it.
test("a completed upload keeps focus where it was and inserts through bracketed paste", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  let id: string | undefined;
  try {
    const session = await openAttachmentSession(page, request, `attach-focus-${stamp}`);
    id = session.id;

    // A precondition, not part of what is under test: this client attached
    // after the agent turned bracketed paste on, so on an old tmux it would
    // never have learned the mode the pane is in (see
    // `latchBracketedPaste`), and the markers below would be missing for a
    // reason that has nothing to do with the insertion path.
    await latchBracketedPaste(page, "terminal");
    // Focus deliberately parked outside the terminal before the drop —
    // the sidebar's hosts toggle, the stable non-terminal control now
    // that the back button is gone.
    await page.locator(".hosts-toggle").focus();
    await dispatchPayload(page, "terminal", "drop", {
      entries: [{ name: `focus-${stamp}.txt`, mime: "text/plain", content: "x" }],
    });
    await waitForIslandMatch(
      page,
      "terminal",
      new RegExp(`(${hostPathEnding(`focus-${stamp}\\.txt`)})`),
    );

    expect(
      await page.evaluate(() => document.activeElement?.className ?? ""),
      "an upload completing must not move the caret",
    ).toContain("hosts-toggle");
    // `^[[200~` is the pty echoing the bracketed-paste start marker back
    // in caret notation; it is only there if the insertion went through
    // the paste path rather than being written straight at the socket.
    expect(await islandLogicalText(page, "terminal")).toContain("^[[200~");
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// The indicator is overlaid rather than placed in the pane's flex column,
// and that is a behavioral choice, not a cosmetic one: a line that took
// height from the terminal would resize the pty every time anyone attached
// anything — twice per upload — reflowing the scrollback and making a
// full-screen agent redraw itself.
test("the attachment indicator never resizes the terminal", async ({ page, request }) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  let id: string | undefined;
  try {
    await page.route("**/api/sessions/*/attachments*", async (route) => {
      await new Promise((resolve) => setTimeout(resolve, 2_000));
      await route.continue();
    });
    const session = await openAttachmentSession(page, request, `attach-geometry-${stamp}`);
    id = session.id;

    const geometry = () =>
      page.evaluate(() => {
        const term = (window as any).__farhelmIslands["terminal"].term;
        return { cols: term.cols, rows: term.rows };
      });
    const before = await geometry();

    await dispatchPayload(page, "terminal", "drop", {
      entries: [{ name: `geometry-${stamp}.txt`, mime: "text/plain", content: "x" }],
    });
    await expect(page.locator('[data-terminal="agent"] .attach-busy')).toBeVisible();
    expect(await geometry(), "the indicator must not take rows from the terminal").toEqual(before);

    await waitForIslandMatch(
      page,
      "terminal",
      new RegExp(`(${hostPathEnding(`geometry-${stamp}\\.txt`)})`),
    );
    expect(await geometry(), "and it must not give them back either").toEqual(before);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// A transfer belongs to the terminal that received it, and so does its
// failure. With several terminals on screen, an error reported anywhere
// but the receiving pane would have the user looking for a file they
// dropped somewhere else.
test("an attachment failure stays in the terminal that received it", async ({ page, request }) => {
  test.setTimeout(150_000);
  const stamp = Date.now();
  let id: string | undefined;
  try {
    await page.route("**/api/sessions/*/attachments*", async (route) => {
      await fulfillAsHelm(route, { status: 500, contentType: "text/plain", body: "refused" });
    });
    const session = await openAttachmentSession(page, request, `attach-isolation-${stamp}`);
    id = session.id;

    const tabId = await addTab(page, 0);
    const island = `terminal-${tabId}`;
    await waitForIslandMounted(page, island);
    await selectTerminal(page, tabId);

    await dispatchPayload(page, island, "drop", {
      entries: [{ name: `tabbed-${stamp}.txt`, mime: "text/plain", content: "x" }],
    });

    await expect(page.locator(`.terminal-pane[data-terminal="${tabId}"] .attach-error`))
      .toContainText(`tabbed-${stamp}.txt`);
    await expect(page.locator('[data-terminal="agent"] .attach-error')).toHaveCount(0);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// Dropping TEXT is not a paste: there is no handler underneath to fall
// through to, so terminal.js inserts it itself — while still preventing
// the default, because the engine's own handling of a dropped URL is to
// navigate there, which would take every terminal on the page down with
// it.
test("a text-only drop is prevented, inserted, and leaves the page where it was", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  const uploads = countUploads(page);
  let id: string | undefined;
  try {
    const session = await openAttachmentSession(page, request, `attach-droptext-${stamp}`);
    id = session.id;
    const before = page.url();

    const result = await dispatchPayload(page, "terminal", "drop", {
      text: `/etc/hosts-${stamp}`,
    });

    expect(result.dragover?.defaultPrevented, "no dragover, no drop").toBe(true);
    expect(result.dragover?.dropEffect, "the island advertises itself as a copy target")
      .toBe("copy");
    expect(result.event.defaultPrevented, "an unprevented text drop navigates the page").toBe(true);
    await waitForIslandLogicalText(page, "terminal", `/etc/hosts-${stamp}`);
    expect(page.url(), "and the page stayed where it was").toBe(before);
    expect(uploads.count(), "dropped text is text").toBe(0);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// Interception has to happen BEFORE xterm's own paste handler, and only
// for the flavors it takes responsibility for. A file paste that reached
// xterm would paste the payload's text on top of the upload; a text paste
// that did NOT reach it would lose the paste entirely.
//
// Both are observed rather than inferred: the helper reports whether the
// event was cancelled, whether propagation was stopped, and whether it
// ever arrived at xterm's own textarea.
test("file pastes are cancelled before xterm sees them; text pastes are not", async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  let id: string | undefined;
  try {
    await page.route("**/api/sessions/*/attachments*", async (route) => {
      await fulfillAsHelm(route, {
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ path: `/tmp/fh-${stamp}/attachments/s/x.txt` }),
      });
    });
    const session = await openAttachmentSession(page, request, `attach-cancel-${stamp}`);
    id = session.id;

    const intercepted = await dispatchPayload(page, "terminal", "paste", {
      entries: [{ name: "doc.txt", mime: "text/plain", content: "x" }],
      text: "would have been pasted",
    });
    expect(intercepted.event.defaultPrevented).toBe(true);
    expect(intercepted.event.propagationStopped).toBe(true);
    expect(
      intercepted.event.reachedTarget,
      "xterm must never see a paste this island took responsibility for",
    ).toBe(false);

    const passed = await dispatchPayload(page, "terminal", "paste", { text: `plain-${stamp}` });
    expect(passed.event.defaultPrevented, "xterm's own handler owns a text paste").toBe(false);
    expect(
      passed.event.reachedTarget,
      "a text paste has to arrive at the textarea xterm listens on, or the paste is simply lost",
    ).toBe(true);
    // Propagation IS stopped for this one — by xterm itself, on the way
    // back up, which is the behavior terminal.js's capture-phase listeners
    // exist to get in front of. What matters here is that it was not
    // stopped BEFORE the target, which `reachedTarget` above is the
    // evidence for.
    await waitForIslandLogicalText(page, "terminal", `plain-${stamp}`);
  } finally {
    if (id) await cleanupSession(request, id);
  }
});

// Some engines expose a payload as a bare `FileList` with no item list at
// all. terminal.js has a fallback for exactly that, and without a test it
// would be reachable only in production — on whichever engine happens to
// take that shape, which is the worst place to discover a typo.
test("a payload with files but no items takes the same path", async ({ page, request }) => {
  test.setTimeout(120_000);
  const stamp = Date.now();
  let id: string | undefined;
  try {
    const session = await openAttachmentSession(page, request, `attach-fileslist-${stamp}`);
    id = session.id;

    await dispatchPayload(page, "terminal", "drop", {
      entries: [
        { name: `listed-${stamp}.txt`, mime: "text/plain", content: "from files", filesOnly: true },
      ],
    });

    const match = await waitForIslandMatch(
      page,
      "terminal",
      new RegExp(`(${hostPathEnding(`listed-${stamp}\\.txt`)})`),
    );
    expect(fs.readFileSync(match[1], "utf8")).toBe("from files");
  } finally {
    if (id) await cleanupSession(request, id);
  }
});
