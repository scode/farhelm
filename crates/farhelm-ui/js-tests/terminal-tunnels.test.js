// Exercise the actual shared terminal exports. Browser e2e cannot enable the
// native capability, so these cases pin classification and dispatch without
// manufacturing a browser-only tunnel permission.
const { test } = require('node:test');
const assert = require('node:assert/strict');
const { tunnelTarget, shouldTunnel, activateWebLink, createLinkGesture } = require('../assets/terminal-links.js');

test('only normalized loopback and wildcard bind targets receive tunnel offers', () => {
  for (const host of ['localhost', 'sub.localhost.', '127.0.0.1', '127.255.1.2', '127.1', '0x7f000001', '0.0.0.0', '[::1]', '[::]']) {
    assert.deepEqual(tunnelTarget(`http://${host}:5173/path?q=1#part`), { port: 5173 }, host);
  }
  assert.deepEqual(tunnelTarget('HTTPS://LOCALHOST/path'), { port: 443 });
  assert.deepEqual(tunnelTarget('http://localhost/path'), { port: 80 });
  for (const host of ['192.168.1.1', '10.0.0.1', '0.1.2.3', 'server', 'localhost.example.test', 'example.local', '[::ffff:127.0.0.1]', '[fe80::1]', 'example.test']) {
    assert.equal(tunnelTarget(`http://${host}:5173/`), null, host);
  }
  for (const uri of ['http://localhost:0/', 'http://localhost:000/', 'http://localhost:65536/', 'http://localhost:bad/',
    'http://', '//localhost/', 'http:localhost', 'file://localhost/x', 'ftp://localhost/',
    ' http://localhost/', 'http://localhost/\npath', 'http://localhost/\uFEFF', 'http://localhost/\x7f',
    `http://localhost/${'x'.repeat(4096)}`]) {
    assert.equal(tunnelTarget(uri), null, JSON.stringify(uri));
  }
});

test('native remote primary Meta release owns the tunnel gesture exclusively', () => {
  const valid = { uri: 'http://localhost:5173/', native: true, remote: true, button: 0,
    metaKey: true, ctrlKey: false, shiftKey: false, altKey: false, click: true, selection: false, mode: 'intercept' };
  assert.equal(shouldTunnel(valid), true);
  assert.equal(shouldTunnel({ ...valid, mode: 'both' }), true);
  for (const change of [{ native: false }, { remote: false }, { button: 1 }, { button: 2 }, { metaKey: false },
    { ctrlKey: true }, { shiftKey: true }, { altKey: true }, { click: false }, { selection: true },
    { mode: 'program' }, { uri: 'https://example.test/' }]) {
    assert.equal(shouldTunnel({ ...valid, ...change }), false, JSON.stringify(change));
  }
  const opened = [];
  const tunneled = [];
  const route = options => activateWebLink(options, uri => opened.push(uri), uri => tunneled.push(uri));
  assert.equal(route(valid), 'tunnel');
  assert.deepEqual(opened, []);
  assert.deepEqual(tunneled, [valid.uri]);
  assert.equal(route({ ...valid, metaKey: false }), 'raw');
  assert.equal(route({ ...valid, native: false }), 'raw');
  assert.deepEqual(opened, [valid.uri, valid.uri]);
  for (const change of [{ click: false }, { selection: true }, { mode: 'program' }, { uri: 'javascript:alert(1)' }]) {
    assert.equal(route({ ...valid, ...change }), 'ignored');
  }
  assert.equal(opened.length, 2);
  assert.equal(tunneled.length, 1);
  // OSC keeps its existing caller-owned http(s) validation. The tunnel's
  // strict raw-input rule must not change ordinary OSC navigation.
  assert.equal(route({ ...valid, uri: 'http:localhost', validated: true }), 'raw');
  assert.equal(opened.at(-1), 'http:localhost');
});

test('a reporting-program drag cannot acquire a tunnel release token', () => {
  const gesture = createLinkGesture();
  gesture.observe({ type: 'mousedown', button: 0, clientX: 10, clientY: 10 }, true);
  gesture.observe({ type: 'mousemove', clientX: 30, clientY: 10 }, true);
  const release = { type: 'mouseup', button: 0, clientX: 10, clientY: 10 };
  gesture.observe(release, true);
  assert.equal(gesture.isClick(release), false);
  assert.equal(shouldTunnel({ uri: 'http://localhost:5173/', native: true, remote: true,
    button: 0, metaKey: true, click: gesture.isClick(release), mode: 'intercept' }), false);
});
