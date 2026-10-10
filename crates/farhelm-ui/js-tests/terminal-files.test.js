// Exercise the shipped recognizer and buffer mapping, not copied detector rules.
const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const { pathTokens, oscTarget, bufferLinks, boundedBlob, MAX_BYTES } = require('../assets/terminal-files.js');

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

/** Enabling xterm all-scheme delivery must never send executable targets to an opener. */
test('OSC allows web unchanged and decoded file paths only', () => {
  assert.deepEqual(oscTarget('https://example.test/x'), { kind: 'web', uri: 'https://example.test/x' });
  assert.deepEqual(oscTarget('file://another-host/tmp/my%20report.pdf'), { kind: 'file', path: '/tmp/my report.pdf' });
  assert.deepEqual(oscTarget('file:///work/link/../report.pdf'), { kind: 'file', path: '/work/link/../report.pdf' });
  assert.deepEqual(oscTarget('file:///work/link/%2e%2e/report.pdf'), { kind: 'file', path: '/work/link/../report.pdf' });
  for (const uri of ['javascript:alert(1)', 'data:text/html,<script>', 'ftp://host/a',
    'file://host?next=/tmp/report.pdf', 'file://host#/tmp/report.pdf', 'file://?next=/tmp/report.pdf', 'file://#/tmp/report.pdf', 'file:///a?', 'file:///a#', 'file:///a%00b', 'file:///a?x', 'file:///a#x', 'file:///bad%']) assert.equal(oscTarget(uri), null, uri);
});

/** Public cell fixtures represent Unicode/wrapping contracts without private xterm internals. */
function terminal(rows) {
  return { buffer: { active: { getLine(y) {
    const row = rows[y];
    if (!row) return undefined;
    return { isWrapped: row.wrapped, length: row.cells.length, getCell(x) {
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

/** Run the shipped addon with an owned DOM and clock, without changing globals.
 * Only notice, click and save primitives are modeled; xterm geometry and real
 * browser rendering remain the two-engine integration suite's responsibility.
 */
function noticeFixture(native = true, opened = true, defaultTimers = false) {
  const children = [];
  const listeners = new Map();
  const timers = new Map();
  let timerId = 0;
  function element() {
    const classes = new Set();
    const attributes = new Map();
    return {
      textContent: '',
      classList: { add: c => classes.add(c), remove: c => classes.delete(c), contains: c => classes.has(c) },
      setAttribute: (k, v) => attributes.set(k, v),
      getAttribute: k => attributes.get(k),
      click() {}, remove() {},
    };
  }
  const root = {
    appendChild: child => children.push(child),
    querySelector: selector => children.find(child => child.className?.split(' ').includes(selector.replace(':scope > .', ''))),
    addEventListener: (name, callback) => listeners.set(name, callback),
    removeEventListener: (name, callback) => { if (listeners.get(name) === callback) listeners.delete(name); },
  };
  const clock = {
    setTimeout(callback, delay) { const id = ++timerId; timers.set(id, { callback, delay }); return id; },
    clearTimeout: id => timers.delete(id),
  };
  const context = vm.createContext({
    window: { farhelmTerminalLinks: { visibleFormatCharacters: text => text } },
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
  const addon = new context.window.farhelmTerminalFiles.FileLinksAddon({ endpoint: '/files', secret: () => 'fixture', native, ...(defaultTimers ? {} : clock) });
  const term = { element: opened ? root : undefined, registerLinkProvider: () => ({ dispose() {} }), getSelection: () => '' };
  addon.activate(term);
  addon.hovered = { path: 'report.pdf', info: { status: 'ready', path: '/output/report.pdf' }, controller: new AbortController() };
  return { addon, timers, listeners, open: () => { term.element = root; }, notice: () => root.querySelector(':scope > .terminal-file-message') };
}

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
    assert.match(fixture.notice().textContent, native ? /^saved to / : /^downloaded /);
    assert.equal(fixture.notice().classList.contains('showing'), true);
    const [id, timer] = [...fixture.timers].find(([, timer]) => timer.delay === 5000);
    fixture.timers.delete(id);
    timer.callback();
    assert.equal(fixture.notice().classList.contains('showing'), false);
    assert.equal(fixture.notice().getAttribute('aria-hidden'), 'true');
    fixture.addon.request = async () => { throw new Error('read sentinel'); };
    await fixture.addon.download('report.pdf');
    assert.equal(fixture.notice().textContent, 'download failed: read sentinel');
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
