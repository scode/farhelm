// Exercise the shipped recognizer and buffer mapping, not copied detector rules.
const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const { spawnSync } = require('node:child_process');
const { linkRangeText } = require('../assets/terminal-links.js');
const { pathTokens, oscTarget, bufferLinks, boundedBlob, formatSize, sizeTier, fileIcon, disclosureFits, markedCoverRects, LINK_COVER_SELECTOR, MAX_BYTES } = require('../assets/terminal-files.js');

/** A noninteractive clipped or covered disclosure is not inspectable. Both
 * dimensions and live cover rectangles matter, including changes after lookup.
 */
test('only a visible fully fitting disclosure can authorize a transfer', () => {
  const rect = { top: 10, left: 10, bottom: 70, right: 210 };
  const viewport = { innerHeight: 100, innerWidth: 300 };
  const fits = { hidden: false, clientHeight: 60, scrollHeight: 60, clientWidth: 200, scrollWidth: 200,
    getBoundingClientRect: () => rect };
  assert.equal(disclosureFits(fits, viewport), true);
  for (const change of [{ hidden: true }, { clientHeight: 0 }, { clientWidth: 0 }, { scrollHeight: 61 }, { scrollWidth: 201 }]) {
    assert.equal(disclosureFits({ ...fits, ...change }, viewport), false);
  }
  for (const edge of [{ top: -1 }, { left: -1 }, { bottom: 101 }, { right: 301 }]) {
    assert.equal(disclosureFits({ ...fits, getBoundingClientRect: () => ({ ...rect, ...edge }) }, viewport), false,
      'content can fit internally while its fixed position is outside the viewport');
  }
  assert.equal(disclosureFits(fits, { ...viewport, innerHeight: 69 }), false, 'a resize rechecks live viewport dimensions');
  assert.equal(disclosureFits(null, viewport), false);
  for (const cover of [
    { top: 20, bottom: 40, left: 200, right: 240 },
    { top: 0, bottom: 100, left: 0, right: 300 },
    { top: 69, bottom: 80, left: 20, right: 30 },
  ]) assert.equal(disclosureFits(fits, viewport, [cover]), false, 'even a partial cover can hide the target');
  for (const cover of [
    { top: 70, bottom: 80, left: 20, right: 30 },
    { top: 20, bottom: 40, left: 210, right: 240 },
    { top: 20, bottom: 20, left: 20, right: 30 },
    { top: 20, bottom: 40, left: 20, right: 20 },
  ]) assert.equal(disclosureFits(fits, viewport, [cover]), true, 'touching edges or an empty region covers nothing');
});

/** Covers are whatever surfaces mark themselves, never a selector list kept
 * here: a hand-kept list is how the quick-launch pill went uncounted. Only a
 * rendered, visible marked surface can hide a destination, so an idle hidden
 * tooltip or a `visibility: hidden` surface must not refuse links, while
 * every other marked surface contributes its live rectangle.
 */
test('the cover check measures every rendered marked surface and nothing else', () => {
  const surface = (name, { rendered = true, visibility = 'visible' } = {}) => ({
    name, visibility,
    getClientRects: () => (rendered ? [{}] : []),
    getBoundingClientRect: () => ({ name, top: 1, left: 2, bottom: 3, right: 4 }),
  });
  const marked = [
    surface('approval-region'), surface('pill'),
    surface('idle-tooltip', { rendered: false }), surface('hidden-notice', { visibility: 'hidden' }),
  ];
  const queried = [];
  const doc = {
    querySelectorAll: (selector) => { queried.push(selector); return marked; },
    defaultView: { getComputedStyle: element => ({ visibility: element.visibility }) },
  };
  assert.deepEqual(markedCoverRects(doc).map(rect => rect.name), ['approval-region', 'pill']);
  assert.deepEqual(queried, [LINK_COVER_SELECTOR], 'the marker is the only thing the check selects on');
  assert.equal(LINK_COVER_SELECTOR, '[data-link-cover]', 'Rust and JS surfaces set this exact attribute');
  doc.querySelectorAll = () => [];
  assert.deepEqual(markedCoverRects(doc), [], 'no marked surface means no cover');
});

/** Decimal display and exact byte thresholds must agree with the host cap;
 * rounding a just-refused file below 100 MB would contradict its refusal.
 */
test('file sizes and tiers use decimal boundaries including the exact cap', () => {
  for (const [bytes, text, tier] of [
    [0, '0 B', 'small'], [999, '999 B', 'small'], [4800, '4.8 KB', 'small'],
    [999999, '1000 KB', 'small'], [1000000, '1 MB', 'medium'],
    [8400000, '8.4 MB', 'medium'], [10000000, '10 MB', 'large'],
    [87000000, '87 MB', 'large'], [100000000, '100 MB', 'large'],
  ]) {
    assert.equal(formatSize(bytes), text);
    assert.equal(sizeTier({ status: 'ready', size: bytes }), tier);
  }
  assert.equal(formatSize(100000001), '100 MB');
  for (const status of ['too_large', 'folder', 'not_found', 'not_readable', 'not_regular']) {
    assert.equal(sizeTier({ status, size: 1 }), 'danger', 'a refusal overrides its size');
  }
});

/** Icons describe file kinds only. An unknown extension stays generic rather
 * than acquiring a document-opening claim from its visual similarity.
 */
test('file icons distinguish documents, PDFs, images, archives and folders', () => {
  assert.equal(fileIcon('/work/notes.MD', 'ready'), '\uf15c');
  assert.equal(fileIcon('/work/main.rs', 'ready'), '\uf15c');
  assert.equal(fileIcon('/work/report.pdf', 'ready'), '\uf1c1');
  assert.equal(fileIcon('/work/shot.png', 'ready'), '\uf1c5');
  assert.equal(fileIcon('/work/files.tar.gz', 'ready'), '\uf1c6');
  assert.equal(fileIcon('/work/notes.md', 'folder'), '\uf07b');
  assert.equal(fileIcon('/work/notes.txt.command', 'ready'), '\uf15b');
});

/** A server's answer cannot make a browser gesture open a local file. Native
 * hints follow the supplied capability, without a second JS extension list.
 */
test('hover preserves both path pieces and advertises open only for native admitted types', async () => {
  for (const [native, isMac, canOpen, hint] of [
    [false, true, true, null], [true, true, true, '⌘-click to open'],
    [true, false, true, 'Ctrl-click to open'], [true, true, false, null],
  ]) {
    const fixture = hoverFixture(native, isMac);
    const pending = fixture.addon.hover({}, 'notes.txt', hoverLink());
    fixture.requests[0].reply({ status: 'ready', host: 'host', path: '/work/notes.txt', size: 4800, can_open: canOpen });
    await pending;
    const [name, directory, detail] = fixture.shown.at(-1).lines;
    assert.equal(name.text, 'notes.txt');
    assert.equal(directory.text, 'host:/work/');
    assert.equal(detail.text, '4.8 KB');
    assert.equal(detail.suffix.includes('click to open'), hint !== null);
    if (hint) assert.ok(detail.suffix.includes(hint));
    fixture.addon.dispose();
  }
});

/** Both xterm adapters pass the click event to this shared admission path.
 * A native modifier can request an opening offered by the hover, while the
 * server can still refuse it after saving; web modifiers only download.
 */
test('modifier opening is native-only and save outcomes survive opener refusal', async () => {
  for (const [native, isMac, event, opens] of [
    [true, true, { metaKey: true }, true], [true, true, { ctrlKey: true }, false],
    [true, false, { ctrlKey: true }, true], [true, false, { metaKey: true }, false],
    [false, true, { metaKey: true }, false], [false, false, { ctrlKey: true }, false],
    [true, true, {}, false],
  ]) {
    const fixture = noticeFixture(native, true, false, isMac);
    let requested;
    fixture.addon.request = async (operation, path, _signal, open) => {
      requested = { operation, path, open };
      return native ? { json: async () => ({ path: '/Downloads/report.pdf', opened: false, open_error: 'only document types open' }) } : new Response('bytes');
    };
    await fixture.addon.download('report.pdf', event);
    assert.deepEqual(requested, { operation: 'download', path: '/output/report.pdf', open: opens });
    assert.match(fixture.notice().textContent, native ? /^Saved to / : /^Downloaded /);
    if (opens) assert.match(fixture.notice().textContent, /not opened: only document types open/);
    assert.equal(fixture.addon.failureMessage, false);
    fixture.addon.dispose();
  }
  const success = noticeFixture(true, true, false, true);
  success.addon.request = async () => ({ json: async () => ({ path: '/Downloads/notes.md', opened: true }) });
  await success.addon.download('report.pdf', { metaKey: true });
  assert.equal(success.notice().textContent, 'Saved to /Downloads/notes.md · opened');
  success.addon.dispose();
});

/** A later policy change cannot authorize an opening the hover never offered.
 * Missing capability data also stays save-only, and that successful save must
 * explain the unfulfilled modifier rather than becoming a download failure.
 */
test('a native modified click saves without opening when hover offered only download', async () => {
  for (const isMac of [true, false]) {
    for (const canOpen of [false, undefined]) {
      const fixture = noticeFixture(true, true, false, isMac);
      fixture.addon.hovered.info.can_open = canOpen;
      let requested;
      fixture.addon.request = async (operation, path, _signal, open) => {
        requested = { operation, path, open };
        return { json: async () => ({ path: '/Downloads/report.pdf' }) };
      };
      await fixture.addon.download('report.pdf', isMac ? { metaKey: true } : { ctrlKey: true });
      assert.deepEqual(requested, { operation: 'download', path: '/output/report.pdf', open: false });
      assert.equal(fixture.notice().textContent, 'Saved to /Downloads/report.pdf · not opened: opening was not offered');
      assert.equal(fixture.addon.failureMessage, false);
      await fixture.addon.download('report.pdf');
      assert.equal(fixture.notice().textContent, 'Saved to /Downloads/report.pdf', 'an ordinary save needs no opening explanation');
      fixture.addon.dispose();
    }
  }
});

/** A ready lookup and a native modifier cannot authorize a drag. The common
 * gesture gate must stop before either download or default-app dispatch.
 */
test('file transfers refuse gestures the terminal did not admit as clicks', async () => {
  for (const native of [false, true]) {
    const fixture = noticeFixture(native, true, false, true);
    fixture.addon.isClick = () => false;
    let requests = 0;
    fixture.addon.request = async () => { requests++; throw new Error('unexpected transfer'); };
    await fixture.addon.download('report.pdf', { metaKey: true });
    assert.equal(requests, 0);
    fixture.addon.dispose();
  }
});

/** The positive/negative table pins prose boundaries and the existing URL policy. */
test('paths exclude diagnostic locations, prose punctuation, URLs and versions', () => {
  for (const [input, path] of [
    ['/srv/out/report.csv', '/srv/out/report.csv'], ['~/out.csv', '~/out.csv'],
    ['src/main.rs:42:7', 'src/main.rs'], ['./build/app', './build/app'],
    ['notes.txt', 'notes.txt'], ['notes.txt:42:7).', 'notes.txt'], ['(report.pdf).', 'report.pdf'], ['"src/main.rs:42",', 'src/main.rs'],
    ['路径/报告.csv!', '路径/报告.csv'],
  ]) assert.deepEqual(pathTokens(input).map(x => x.path), [path], input);
  for (const input of ['hello', '1.2.3', 'v1.2.3', '//host/out.csv', 'https://host/out.csv',
    'ftp://host/out.csv', 'file:///tmp/out.csv', 'data:report.pdf', 'mailto:a@host.example',
    'javascript:alert/report.pdf', 'https:report.pdf', 'javascript:alert(document.domain)', 'ftp://host/(src/main.rs)', '(javascript:alert(document.domain))']) assert.deepEqual(pathTokens(input), [], input);
});

/** Digit-leading extensions must not turn numeric versions or quantities into
 * file evidence. A slash still makes an ambiguous token an explicit path.
 */
test('bare digit-leading extensions retain numeric-token exclusions', () => {
  for (const input of ['backup.7z', 'backup.001', 'archive.1part', '123.txt', './1.5x']) {
    assert.deepEqual(pathTokens(input).map(token => token.path), [input], input);
  }
  for (const input of ['1.5x', '-1.5x', '+1.5x', 'v1.5x', '1.2rc1', '1.2.3', 'v1.2.3', '1.2.3-rc1', '1.2+build']) {
    assert.deepEqual(pathTokens(input), [], input);
  }
});

/** Numeric-looking file stems must finish recognition even when the numeric
 * guard nearly matches. A separate process bounds a regressed synchronous
 * regex without hanging the harness; its assertion also pins the file result.
 */
test('a long numeric-looking filename completes recognition', () => {
  const result = spawnSync(process.execPath, ['-e', `
    const assert = require('node:assert/strict');
    const { pathTokens } = require(process.argv[1]);
    const input = '1' + '.1111111111'.repeat(8) + '.x';
    assert.deepEqual(pathTokens(input).map(token => token.path), [input]);
  `, require.resolve('../assets/terminal-files.js')], { timeout: 5000, encoding: 'utf8' });
  assert.equal(result.error, undefined, result.error?.message);
  assert.equal(result.status, 0, result.stderr);
});

/** Ordinary release-version spellings stay out of file links, and the guard
 * that keeps them out answers a long near-match in linear time: `1.1`,
 * thousands of hyphens and a stray `@` once cost 50 ms or more per call. Five
 * hundred calls fit the subprocess timeout only when each takes under 10 ms.
 */
test('the numeric-version guard keeps version spellings out and answers near-matches fast', () => {
  for (const input of ['1.2-rc.3', 'v1.5x', '2.0.1+build.5', '1.2.3-beta-2', '1.2.3-rc.1+build.5', '1.0.0-beta+exp.sha.5114f85', '1.2-']) {
    assert.deepEqual(pathTokens(input), [], input);
  }
  const result = spawnSync(process.execPath, ['-e', `
    const assert = require('node:assert/strict');
    const { NUMERIC_VERSION } = require(process.argv[1]);
    const near = '1.1' + '-'.repeat(8188) + '@';
    for (let i = 0; i < 500; i++) assert.equal(NUMERIC_VERSION.test(near), false);
  `, require.resolve('../assets/terminal-files.js')], { timeout: 5000, encoding: 'utf8' });
  assert.equal(result.error, undefined, result.error?.message);
  assert.equal(result.status, 0, result.stderr);
});

/** Enabling xterm all-scheme delivery must never send executable targets to an opener. */
test('OSC allows web unchanged and decoded file paths only', () => {
  assert.deepEqual(oscTarget('https://example.test/x'), { kind: 'web', uri: 'https://example.test/x' });
  assert.deepEqual(oscTarget('file://another-host/tmp/my%20report.pdf'), { kind: 'file', path: '/tmp/my report.pdf' });
  assert.deepEqual(oscTarget('file:///work/link/../report.pdf'), { kind: 'file', path: '/work/link/../report.pdf' });
  assert.deepEqual(oscTarget('file:///work/link/%2e%2e/report.pdf'), { kind: 'file', path: '/work/link/../report.pdf' });
  for (const uri of ['javascript:alert(1)', 'data:text/html,<script>', 'ftp://host/a',
    'file://host?next=/tmp/report.pdf', 'file://host#/tmp/report.pdf', 'file://?next=/tmp/report.pdf', 'file://#/tmp/report.pdf', 'file:///a?', 'file:///a#', 'file:///a%00b', 'file:///a?x', 'file:///a#x', 'file:///bad%']) assert.equal(oscTarget(uri), null, uri);
});

/** Public cell fixtures represent Unicode/wrapping contracts without private
 * xterm internals. Both recognition and text snapshots read these same cells,
 * so a partial repaint changes one shared buffer rather than two test doubles.
 */
function terminal(rows) {
  return { buffer: { active: { getLine(y) {
    const row = rows[y];
    if (!row) return undefined;
    return { isWrapped: row.wrapped, length: row.cells.length,
    translateToString(trim, from, to) {
      const text = row.cells.slice(from, to).map(([chars, width]) => width ? chars || ' ' : '').join('');
      return trim ? text.trimEnd() : text;
    }, getCell(x) {
      const [chars, width] = row.cells[x];
      return { getChars: () => chars, getWidth: () => width };
    } };
  } } } };
}

/** UTF-16 offsets must not shift a link after wide or combined cells, including wraps. */
test('buffer mapping handles wide, combining and wrapped path cells', () => {
  const cells = s => Array.from(s, ch => [ch, 1]);
  const term = terminal([
    { cells: [['界', 2], ['', 0], ['e\u0301', 1], [' ', 1], ...cells('src/mai')], wrapped: false },
    { cells: cells('n.rs:42!'), wrapped: true },
  ]);
  for (const row of [1, 2]) assert.deepEqual(bufferLinks(term, row), [{
    path: 'src/main.rs', range: { start: { x: 5, y: 1 }, end: { x: 4, y: 2 } },
  }]);
});

/** Opening headers are not completion: bytes are offered only after EOF. */
test('bounded Blob refuses excess bytes and mid-stream errors', async () => {
  assert.equal(await (await boundedBlob(new Response('payload'))).text(), 'payload');
  const failure = new ReadableStream({ start(c) { c.enqueue(new Uint8Array(1)); c.error(new Error('read sentinel')); } });
  await assert.rejects(boundedBlob(new Response(failure)), /read sentinel/);
  const tooLarge = new ReadableStream({ start(c) { c.enqueue(new Uint8Array(MAX_BYTES + 1)); c.close(); } });
  await assert.rejects(boundedBlob(new Response(tooLarge)), /100 MB limit/);
});

/** Isolate the shipped hover state machine with a controllable fetch and task
 * clock. The disclosure sink records every replacement, so preserving its
 * final text alone cannot hide an intermediate spinner or blanking regression.
 */
function hoverFixture(native = false, isMac = false) {
  const tasks = new Map();
  const requests = [];
  const shown = [];
  const display = { hidden: true };
  let timerId = 0;
  const clock = {
    setTimeout(callback) { const id = ++timerId; tasks.set(id, callback); return id; },
    clearTimeout: id => tasks.delete(id),
  };
  const context = vm.createContext({
    window: { farhelmTerminalLinks: { linkRangeText } }, AbortController,
    fetch: (url, options) => new Promise(resolve => {
      requests.push({ url, signal: options.signal, path: JSON.parse(options.body).path,
        reply: info => resolve({ ok: true, json: async () => info }) });
    }),
  });
  vm.runInContext(fs.readFileSync(require.resolve('../assets/terminal-files.js'), 'utf8'), context);
  const addon = new context.window.farhelmTerminalFiles.FileLinksAddon({ endpoint: '/files', secret: () => 'fixture', native, isMac, isClick: () => true, ...clock });
  addon.activate({ element: { querySelector: selector => selector.endsWith('.terminal-file-target') ? display : null, removeEventListener() {} },
    registerLinkProvider: () => ({ dispose() {} }), getSelection: () => '' });
  addon.show = (_event, lines, checking = false) => { display.hidden = false; shown.push({ lines: [...lines], checking }); };
  return { addon, requests, shown, display, tasks,
    runTasks() { for (const [id, callback] of [...tasks]) { tasks.delete(id); callback(); } } };
}

/** Different objects with the same cell endpoints model xterm's re-linkification. */
function hoverLink(startX = 2, startY = 3, endX = 11, endY = 3) {
  return { range: { start: { x: startX, y: startY }, end: { x: endX, y: endY } }, decorations: { pointerCursor: false } };
}

/** A pending stat must survive every same-task redraw, and its answer must
 * admit the replacement link rather than writing through the retired object.
 */
test('same-task redraw preserves a pending hover and updates its newest link', async () => {
  const fixture = hoverFixture();
  const first = hoverLink();
  const pending = fixture.addon.hover({}, 'report.pdf', first);
  const state = fixture.addon.hovered;
  let current;
  for (let redraw = 0; redraw < 3; redraw++) {
    fixture.addon.leave();
    assert.equal(first.decorations.pointerCursor, false);
    current = hoverLink();
    await fixture.addon.hover({}, 'report.pdf', current);
    assert.equal(fixture.addon.hovered, state);
    assert.equal(fixture.tasks.size, 0);
    assert.equal(fixture.requests[0].signal.aborted, false);
    assert.equal(fixture.requests.length, 1);
    assert.equal(fixture.shown.length, 1);
  }
  fixture.requests[0].reply({ status: 'ready', host: 'this machine', path: '/output/report.pdf', size: 8 });
  await pending;
  assert.equal(current.decorations.pointerCursor, true);
  assert.equal(first.decorations.pointerCursor, false);
  assert.equal(fixture.shown.length, 2);
  fixture.addon.dispose();
});

/** Ready and refused answers both belong to this hover; redraw must neither
 * replace the disclosure nor carry a ready cursor onto a refused replacement.
 */
test('same-task redraw preserves answered ready and refused hovers', async () => {
  for (const status of ['ready', 'folder']) {
    const fixture = hoverFixture();
    const first = hoverLink();
    const pending = fixture.addon.hover({}, 'report.pdf', first);
    fixture.requests[0].reply({ status, host: 'this machine', path: '/output/report.pdf', size: 8 });
    await pending;
    const state = fixture.addon.hovered;
    fixture.addon.leave();
    assert.equal(first.decorations.pointerCursor, false, 'leave must remove the old hand synchronously');
    const next = hoverLink();
    await fixture.addon.hover({}, 'report.pdf', next);
    assert.equal(fixture.addon.hovered, state);
    assert.equal(next.decorations.pointerCursor, status === 'ready');
    assert.equal(fixture.requests.length, 1);
    assert.equal(fixture.shown.length, 2);
    assert.equal(fixture.display.hidden, false);
    assert.equal(fixture.tasks.size, 0);
    fixture.addon.dispose();
  }
});

/** xterm installs decoration accessors after the synchronous hover callback.
 * Restoration must reach those accessors, while a departure before the
 * microtask runs must not write through them onto an unrelated current link.
 */
test('ready redraw cursor restoration waits for accessors and rejects retired links', async () => {
  for (const depart of [false, true]) {
    const fixture = hoverFixture();
    const pending = fixture.addon.hover({}, 'report.pdf', hoverLink());
    fixture.requests[0].reply({ status: 'ready', host: 'this machine', path: '/output/report.pdf', size: 8 });
    await pending;
    fixture.addon.leave();
    const next = hoverLink();
    const restoring = fixture.addon.hover({}, 'report.pdf', next);
    let cursor = { pointer: false };
    next.decorations = {};
    Object.defineProperty(next.decorations, 'pointerCursor', { set: value => { cursor.pointer = value; } });
    if (depart) {
      fixture.addon.leave();
      cursor = { pointer: true };
    }
    await restoring;
    assert.equal(cursor.pointer, true, depart ? 'a retired link must not touch the now-current cursor' : 'restore through installed accessors');
    assert.equal(fixture.requests.length, 1);
    fixture.addon.dispose();
  }
});

/** Text alone is not hover identity: each endpoint and path must agree, and
 * no range means identity cannot be established. Replacement cancels old work.
 */
test('different paths, cell endpoints and unknown ranges start fresh checks', async () => {
  for (const [path, next] of [['other.pdf', hoverLink()], ['report.pdf', hoverLink(1)],
    ['report.pdf', hoverLink(2, 2)], ['report.pdf', hoverLink(2, 3, 12)],
    ['report.pdf', hoverLink(2, 3, 11, 4)], ['report.pdf', null]]) {
    const fixture = hoverFixture();
    const pending = fixture.addon.hover({}, 'report.pdf', hoverLink());
    fixture.addon.leave();
    const replacement = fixture.addon.hover({}, path, next);
    assert.equal(fixture.requests.length, 2);
    assert.equal(fixture.requests[0].signal.aborted, true);
    assert.equal(fixture.tasks.size, 0);
    assert.equal(fixture.addon.hovered.path, path);
    fixture.requests[0].reply({ status: 'ready' });
    await pending;
    assert.equal(fixture.shown.length, 2, 'retired evidence cannot overwrite the new spinner');
    fixture.requests[1].reply({ status: 'folder' });
    await replacement;
    fixture.addon.dispose();
  }
});

/** OSC labels are independent of their file target. A changed same-width
 * label, or unavailable display text, ends evidence even at identical cells.
 */
test('changed and unknown OSC display text cancels old evidence at the same target and cells', async () => {
  for (const text of ['new.txt', null]) {
    const fixture = hoverFixture();
    const range = hoverLink().range;
    const pending = fixture.addon.hover({}, '/output/report.pdf', null, range, 'old.txt');
    fixture.addon.leave();
    const replacement = fixture.addon.hover({}, '/output/report.pdf', null, range, text);
    assert.equal(fixture.requests.length, 2);
    assert.equal(fixture.requests[0].signal.aborted, true);
    assert.equal(fixture.tasks.size, 0);
    fixture.requests[0].reply({ status: 'ready' });
    await pending;
    assert.equal(fixture.shown.length, 2, 'old evidence cannot overwrite the replacement spinner');
    fixture.requests[1].reply({ status: 'folder' });
    await replacement;
    fixture.addon.dispose();
  }
});

/** A genuine departure ends evidence at the next task; returning later must
 * check again. Pending replies after that boundary cannot resurrect the tooltip.
 */
test('real departure aborts and hides, and a later same-path hover checks afresh', async () => {
  const fixture = hoverFixture();
  const pending = fixture.addon.hover({}, 'report.pdf', hoverLink());
  fixture.addon.leave();
  assert.equal(fixture.requests[0].signal.aborted, false);
  assert.equal(fixture.tasks.size, 1);
  fixture.runTasks();
  assert.equal(fixture.requests[0].signal.aborted, true);
  assert.equal(fixture.display.hidden, true);
  assert.equal(fixture.addon.hovered, null);
  fixture.requests[0].reply({ status: 'ready' });
  await pending;
  assert.equal(fixture.display.hidden, true);
  const returning = fixture.addon.hover({}, 'report.pdf', hoverLink());
  assert.equal(fixture.requests.length, 2);
  fixture.requests[1].reply({ status: 'folder' });
  await returning;
  fixture.addon.dispose();
});

/** xterm cursor accessors write to its current link, not necessarily the
 * object that carries them. A deferred teardown must never touch that setter.
 */
test('leave withdraws admission immediately and deferred teardown leaves another cursor alone', async () => {
  const fixture = hoverFixture();
  let cursor = { pointer: false };
  const first = hoverLink();
  Object.defineProperty(first.decorations, 'pointerCursor', { set: value => { cursor.pointer = value; } });
  const pending = fixture.addon.hover({}, 'report.pdf', first);
  fixture.requests[0].reply({ status: 'ready', path: '/output/report.pdf' });
  await pending;
  assert.equal(cursor.pointer, true);
  fixture.addon.leave();
  assert.equal(cursor.pointer, false);
  await fixture.addon.download('report.pdf');
  assert.equal(fixture.requests.length, 1, 'a leaving hover must already refuse a click');
  cursor = { pointer: true };
  fixture.runTasks();
  assert.equal(cursor.pointer, true, 'the now-current unrelated link must keep its hand');
  fixture.addon.dispose();
});

/** Terminal disposal is immediate even during the redraw grace task; it
 * owns the task as well as the request and rejects later replies.
 */
test('disposal clears pending hover teardown and rejects late replies', async () => {
  const fixture = hoverFixture();
  const pending = fixture.addon.hover({}, 'report.pdf', hoverLink());
  fixture.addon.leave();
  fixture.addon.dispose();
  assert.equal(fixture.requests[0].signal.aborted, true);
  assert.equal(fixture.tasks.size, 0);
  assert.equal(fixture.display.hidden, true);
  fixture.requests[0].reply({ status: 'ready' });
  await pending;
  assert.equal(fixture.shown.length, 1);
});

/** Run the shipped addon with an owned DOM and clock, without changing globals.
 * Notice, click, save and public buffer primitives are modeled; real geometry and
 * browser rendering remain the two-engine integration suite's responsibility.
 * The ready hover offers opening by default; save-only cases remove that offer.
 */
function noticeFixture(native = true, opened = true, defaultTimers = false, isMac = false) {
  const children = [];
  const listeners = new Map();
  const timers = new Map();
  const disclosure = { hidden: false, clientHeight: 60, scrollHeight: 60, clientWidth: 200, scrollWidth: 200,
    getBoundingClientRect: () => ({ top: 10, left: 10, bottom: 70, right: 210 }) };
  let timerId = 0;
  function element() {
    const classes = new Set();
    const attributes = new Map();
    return {
      textContent: '',
      dataset: {},
      classList: { add: c => classes.add(c), remove: c => classes.delete(c), contains: c => classes.has(c) },
      setAttribute: (k, v) => attributes.set(k, v),
      getAttribute: k => attributes.get(k),
      click() {}, remove() {},
    };
  }
  const root = {
    appendChild: child => children.push(child),
    querySelector: selector => selector === ':scope > .terminal-file-target' ? disclosure :
      children.find(child => child.className?.split(' ').includes(selector.replace(':scope > .', ''))),
    addEventListener: (name, callback) => listeners.set(name, callback),
    removeEventListener: (name, callback) => { if (listeners.get(name) === callback) listeners.delete(name); },
  };
  const clock = {
    setTimeout(callback, delay) { const id = ++timerId; timers.set(id, { callback, delay }); return id; },
    clearTimeout: id => timers.delete(id),
  };
  const context = vm.createContext({
    window: { innerHeight: 100, innerWidth: 300, farhelmTerminalLinks: { visibleFormatCharacters: text => text, linkRangeText } },
    document: { createElement: element },
    AbortController, Blob,
    // A strict callable rejects an addon receiver just as browser timers do.
    // The default wrappers must invoke these as globals, not instance methods.
    setTimeout: function(callback, delay) {
      'use strict';
      assert.equal(this, undefined);
      return clock.setTimeout(callback, delay);
    },
    clearTimeout: function(id) {
      'use strict';
      assert.equal(this, undefined);
      return clock.clearTimeout(id);
    },
    URL: { createObjectURL: () => 'blob:owned', revokeObjectURL() {} },
  });
  vm.runInContext(fs.readFileSync(require.resolve('../assets/terminal-files.js'), 'utf8'), context);
  const addon = new context.window.farhelmTerminalFiles.FileLinksAddon({ endpoint: '/files', secret: () => 'fixture', native, isMac, isClick: () => true, ...(defaultTimers ? {} : clock) });
  const term = { element: opened ? root : undefined, registerLinkProvider: () => ({ dispose() {} }), getSelection: () => '',
    buffer: { active: { getLine: () => ({ length: 10, translateToString: () => 'report.pdf' }) } } };
  addon.activate(term);
  addon.hovered = { path: 'report.pdf', info: { status: 'ready', path: '/output/report.pdf', can_open: true }, controller: new AbortController(),
    range: { start: { x: 1, y: 1 }, end: { x: 10, y: 1 } }, screenText: 'report.pdf' };
  return { addon, timers, listeners, disclosure, open: () => { term.element = root; }, notice: () => root.querySelector(':scope > .terminal-file-message') };
}

/** A fitting stat is insufficient once the window clips its disclosure.
 * Refusal must precede any transfer/open request, and its explanation must
 * survive the activation click while still obeying notice teardown.
 */
test('clipping refuses transfer and shows failure after its own click', async () => {
  for (const native of [false, true]) {
    const fixture = noticeFixture(native, true, false, true);
    fixture.disclosure.scrollHeight++;
    let requests = 0;
    fixture.addon.request = async () => { requests++; throw new Error('unexpected transfer'); };
    await fixture.addon.download('report.pdf', { metaKey: true });
    assert.equal(requests, 0);
    assert.equal(fixture.notice(), undefined, 'mouseup has not shown a self-dismissing notice');
    assert.equal(fixture.timers.size, 1);
    const [id, task] = [...fixture.timers][0];
    assert.equal(task.delay, 0);
    fixture.listeners.get('click')?.();
    fixture.timers.delete(id);
    task.callback();
    assert.match(fixture.notice().textContent, /full file details are not visible/);
    assert.equal(fixture.notice().dataset.outcome, 'disclosure-refusal');
    assert.equal(fixture.notice().classList.contains('showing'), true);
    // A new hover or leaving lowers it to the failure layer, keeping its text.
    fixture.addon.clearHover();
    assert.equal(fixture.notice().dataset.outcome, 'failure');
    assert.match(fixture.notice().textContent, /full file details are not visible/);
    assert.equal(fixture.notice().classList.contains('showing'), true);
    fixture.listeners.get('click')();
    assert.equal(fixture.notice().classList.contains('showing'), false);
    fixture.addon.message('downloading…');
    assert.equal(fixture.notice().dataset.outcome, 'progress', 'a later notice cannot inherit refusal stacking');
    const pending = noticeFixture(native, true, false, true);
    pending.disclosure.scrollHeight++;
    await pending.addon.download('report.pdf', { metaKey: true });
    assert.equal(pending.timers.size, 1);
    pending.addon.dispose();
    assert.equal(pending.timers.size, 0, 'disposal cancels a pending explanation');
  }
});

/** An approval may arrive after lookup. Both activation paths consult its
 * current region; a departed web hover also lowers a not-yet-shown refusal.
 */
test('live approval coverage refuses file transfers and shares web refusal lifetime', async () => {
  const fixture = noticeFixture();
  let cover = { top: 20, bottom: 40, left: 200, right: 240 };
  fixture.addon.coveringRects = () => [cover];
  let transfers = 0;
  fixture.addon.request = async () => { transfers++; throw new Error('unexpected transfer'); };
  await fixture.addon.download('report.pdf', { ctrlKey: true });
  assert.equal(transfers, 0);
  let [id, task] = [...fixture.timers][0];
  fixture.timers.delete(id);
  task.callback();
  assert.match(fixture.notice().textContent, /cannot download:.*covered by another Farhelm surface/);
  assert.equal(fixture.notice().dataset.outcome, 'disclosure-refusal');

  assert.equal(fixture.addon.admitDisclosure(fixture.disclosure, true), false);
  fixture.addon.lowerRefusal();
  [id, task] = [...fixture.timers][0];
  fixture.timers.delete(id);
  task.callback();
  assert.match(fixture.notice().textContent, /cannot open:.*covered by another Farhelm surface/);
  assert.equal(fixture.notice().dataset.outcome, 'failure', 'departure before the task must not re-raise the notice');
  cover = { top: 70, bottom: 80, left: 20, right: 30 };
  assert.equal(fixture.addon.admitDisclosure(fixture.disclosure, true), true, 'moving the card away admits the same hover');
  fixture.addon.dispose();
});

/** A cached link can survive repaint of only its continuation. The actual
 * shipped admission path must reject the changed cells before requesting
 * either a browser transfer or a native save/open, and still accept unchanged
 * cells. Capturing via hover pins the snapshot timing, not just comparison.
 */
test('partial wrapped redraw invalidates file transfer evidence', async () => {
  for (const native of [false, true]) {
    const fixture = noticeFixture(native, true, false, true);
    let rows = ['report', '.pdf'];
    fixture.addon.term.buffer.active.getLine = y => rows[y] === undefined ? undefined : {
      length: rows[y].length,
      translateToString: (_trim, start, end) => rows[y].slice(start, end),
    };
    const range = { start: { x: 1, y: 1 }, end: { x: 4, y: 2 } };
    fixture.addon.show = () => { fixture.disclosure.hidden = false; };
    const requests = [];
    fixture.addon.request = async (operation, _path, _signal, open) => {
      requests.push({ operation, open });
      if (operation === 'stat') return { json: async () => ({ status: 'ready', path: '/output/report.pdf', size: 1, host: 'fixture', can_open: true }) };
      return native ? { json: async () => ({ path: '/Downloads/report.pdf', opened: false }) } : new Response('bytes');
    };
    await fixture.addon.hover({}, 'report.pdf', null, range, 'report.pdf');
    assert.equal(fixture.addon.hovered.screenText, 'report.pdf');
    assert.equal(fixture.addon.hovered.info.status, 'ready');
    rows[1] = '.txt';
    await fixture.addon.download('report.pdf', { metaKey: true });
    assert.equal(requests.length, 1, 'changed cells cannot reuse the stat for a transfer');
    rows[1] = '.pdf';
    await fixture.addon.download('report.pdf', { metaKey: true });
    assert.deepEqual(requests[1], { operation: 'download', open: native });
    rows = [];
    await fixture.addon.download('report.pdf', { metaKey: true });
    assert.equal(requests.length, 2, 'missing rows cannot establish unchanged text');
    fixture.addon.dispose();
  }
});

/** A repaint may append a suffix while leaving every cached link cell intact.
 * Recognition must still identify the complete printed token and its endpoints;
 * comparing the old rectangle alone would download/open the previous filename.
 */
test('a wrapped printed path cannot grow beyond its cached endpoint', async () => {
  for (const native of [false, true]) {
    const fixture = noticeFixture(native, true, false, true);
    const cells = text => Array.from(text, ch => [ch, 1]);
    const rows = [
      { cells: cells('long/repor'), wrapped: false },
      { cells: cells('t.txt     '), wrapped: true },
    ];
    fixture.addon.term.buffer = terminal(rows).buffer;
    const [{ path, range }] = bufferLinks(fixture.addon.term, 1);
    assert.equal(path, 'long/report.txt');
    assert.deepEqual(range.end, { x: 5, y: 2 });
    fixture.addon.show = () => { fixture.disclosure.hidden = false; };
    const requests = [];
    fixture.addon.request = async (operation, _path, _signal, open) => {
      requests.push({ operation, open });
      if (operation === 'stat') return { json: async () => ({ status: 'ready', path: '/output/long/report.txt', size: 1, host: 'fixture', can_open: true }) };
      return native ? { json: async () => ({ path: '/Downloads/report.txt', opened: true }) } : new Response('bytes');
    };
    await fixture.addon.hover({}, path, { range, decorations: {} }, range);
    assert.equal(fixture.addon.hovered.info.status, 'ready');
    rows[1].cells = cells('t.txt.bak ');
    assert.equal(linkRangeText(fixture.addon.term, range), fixture.addon.hovered.screenText,
      'the old rectangle alone still matches after the extension');
    assert.equal(bufferLinks(fixture.addon.term, 1)[0].path, 'long/report.txt.bak');
    await fixture.addon.download(path, { metaKey: true });
    assert.equal(requests.length, 1, 'the newly extended token must not reuse the previous target');
    rows[1].cells = cells('t.txt     ');
    await fixture.addon.download(path, { metaKey: true });
    assert.deepEqual(requests[1], { operation: 'download', open: native });
    fixture.addon.dispose();
  }
});

/** Real browser timers reject an addon as their receiver. Default scheduling
 * and cancellation must retain global-call semantics just like injected clocks.
 */
test('default notice timers do not receive the addon as their receiver', () => {
  const fixture = noticeFixture(true, true, true);
  fixture.addon.message('saved to /output/report.pdf', 'success');
  assert.equal(fixture.timers.size, 1);
  fixture.addon.message('download failed: sentinel', 'failure');
  assert.equal(fixture.timers.size, 0);
  fixture.addon.dispose();
});

/** xterm activates addons before creating its element. Failed mounts must
 * also dispose safely; notice listeners belong to a later, opened terminal.
 */
test('activation and disposal do not require an opened terminal element', () => {
  const failedMount = noticeFixture(true, false);
  assert.equal(failedMount.listeners.size, 0);
  failedMount.addon.dispose();
  const mounted = noticeFixture(true, false);
  mounted.open();
  mounted.addon.message('download failed: sentinel', 'failure');
  assert.equal(mounted.listeners.size, 1);
  mounted.listeners.get('click')();
  assert.equal(mounted.notice().textContent, '');
  mounted.addon.dispose();
});

/** Both save paths have finite success notices; progress and failures have no
 * deadline. Advancing an injected clock proves expiry without a timing guess.
 */
test('browser and native successes fade, while a failure waits for a terminal click', async () => {
  for (const native of [false, true]) {
    const fixture = noticeFixture(native);
    fixture.addon.request = async () => native ? { json: async () => ({ path: '/Downloads/report.pdf' }) } : new Response('bytes');
    await fixture.addon.download('report.pdf');
    assert.match(fixture.notice().textContent, native ? /^Saved to / : /^Downloaded /);
    assert.equal(fixture.notice().classList.contains('showing'), true);
    const [id, timer] = [...fixture.timers].find(([, timer]) => timer.delay === 5000);
    fixture.timers.delete(id);
    timer.callback();
    assert.equal(fixture.notice().classList.contains('showing'), false);
    assert.equal(fixture.notice().getAttribute('aria-hidden'), 'true');
    fixture.addon.request = async () => { throw new Error('read sentinel'); };
    await fixture.addon.download('report.pdf');
    assert.equal(fixture.notice().textContent, 'Download failed: read sentinel');
    assert.equal([...fixture.timers.values()].some(timer => timer.delay > 0), false);
    fixture.listeners.get('click')();
    assert.equal(fixture.notice().textContent, '');
    fixture.addon.dispose();
  }
});

/** An old success must not erase a new progress/failure, and a new download
 * must replace a failure even when invoked without a DOM click (OSC handler).
 */
test('replacement cancels an earlier notice deadline and retires a failure', () => {
  const { addon, timers, notice } = noticeFixture();
  addon.message('saved to first', 'success');
  assert.equal(timers.size, 1);
  addon.message('downloading…');
  assert.equal(timers.size, 0);
  assert.equal(notice().textContent, 'downloading…');
  addon.message('download failed: sentinel', 'failure');
  assert.equal(timers.size, 0);
  addon.message('downloading…');
  assert.equal(addon.failureMessage, false);
  assert.equal(notice().classList.contains('showing'), true);
  addon.dispose();
});

/** Closing a terminal owns timer/listener cleanup and cancels an in-flight
 * native reply; a late save result cannot resurrect its retired notice.
 */
test('terminal disposal clears notices and rejects a late native save result', async () => {
  const { addon, timers, listeners, notice } = noticeFixture();
  addon.message('saved to earlier', 'success');
  addon.dispose();
  assert.equal(timers.size, 0);
  assert.equal(listeners.size, 0);
  assert.equal(notice().textContent, '');
  const pending = noticeFixture();
  let complete;
  let markReading;
  const reading = new Promise(resolve => { markReading = resolve; });
  pending.addon.request = async () => ({ json: () => new Promise(resolve => { complete = resolve; markReading(); }) });
  const transfer = pending.addon.download('report.pdf');
  await reading;
  assert.equal(typeof complete, 'function');
  assert.equal(pending.addon.transfers.size, 1);
  pending.addon.dispose();
  complete({ path: '/Downloads/report.pdf' });
  await transfer;
  assert.equal(pending.notice().textContent, '');
  assert.equal(pending.timers.size, 0);
  assert.equal(pending.addon.transfers.size, 0);
});
