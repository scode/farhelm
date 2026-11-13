#!/usr/bin/env -S deno run -A
// Evaluate the pinned shipping JavaScript, not a second implementation of its
// rules. This generator needs a Farhelm checkout only when refreshing the corpus.
import vm from "node:vm";

const sources = new Map([
  ["terminal-links.js", "b0bfc450d772891fdfa20e8f39394ec1b9af872afb48f8d8ea0d0acbe0c404b6"],
  ["grid-buffer.js", "12cbc27b48f0ec0617005f5e0f6503762bdcdd986da10524a0c3b26a8687d3fb"],
]);

/** Reject source drift before producing new expected answers. */
async function load(root: string, name: string) {
  const bytes = await Deno.readFile(`${root}/crates/farhelm-ui/assets/${name}`);
  const digest = Array.from(
    new Uint8Array(await crypto.subtle.digest("SHA-256", bytes)),
    (b) => b.toString(16).padStart(2, "0"),
  ).join("");
  if (digest !== sources.get(name)) throw new Error(`Wrong pinned ${name}: ${digest}`);
  const module = { exports: {} as Record<string, (...args: any[]) => any> };
  vm.runInNewContext(new TextDecoder().decode(bytes), { module, URL });
  return module.exports;
}

/** Store native-renderable text when a JS middle slice splits a surrogate. */
function nativeText(text: string): string {
  return new TextDecoder().decode(new TextEncoder().encode(text));
}

if (import.meta.main) {
  const root = Deno.args[0];
  if (!root) throw new Error("Usage: generate-links.ts /absolute/path/to/pinned/farhelm");
  const api = await load(root, "terminal-links.js");
  const buffer = await load(root, "grid-buffer.js");
  const targets = [
    "https://example.com/x",
    "https://example.com/x/",
    "http://example.com/x",
    "https://example.com/x?",
    "https://example.com/x#",
    "https://example.com/x?#",
    "https://example.com/%C3%BC",
    "https://example.com/?next=/",
    "https://example.com/?next=",
    "https://example.com/a#/",
    "https://example.com/a#",
    "https://example.com:443/x",
    "https://example.com:8443/x",
    "https://good.example.evil.test/",
    "https://github.com@evil.example/",
    "https://:secret@example.com/x",
    "https://user:secret@example.com/x",
    "https://exаmple.com/",
    "https://[::1]:8443/x",
    "http://127.1/x",
    "https://example.com/😀",
    "https://example.com/a\u202Eb",
    "file:///tmp/x",
    "javascript:alert(1)",
    "not a url",
  ];
  const labels = [
    ...targets,
    "example.com/x",
    "www.example.com",
    "docs.rs/foo",
    "https://good.example",
    "HTTPS://Example.COM/x",
    "https://example.com/%c3%bc",
    "https://exa mple.com/",
    "httрs://github.com/login",
    "click here",
    "Visit https://evil.example",
    "#123",
    "main.rs",
    "setup.py",
    "src/main.rs",
    "v1.2/CHANGELOG.md",
    "changelog.d/x.md",
    "1.2.3/",
    "127.0.0.1:6080/x",
    ...["\u2800", "\u3164", "\uFFA0", "\uFEFF", "\u0085", "\u034F", "\u202E"].map((c) =>
      `${c}https://github.com/login`
    ),
    ...["\u02D0", "\u02F8", "\u2236", "\uA789", "\uFF1A", "\uFE13", "\uFE55"].map((c) => `https${c}//github.com/login`),
    ...["\u2215", "\u2044", "\u29F8", "\uFF0F", "\u2571"].map((c) => `github.com${c}login`),
    ...["\u3002", "\uFF0E", "\uFF61", "\u2024"].map((c) => `github${c}com/login`),
    "https://exa\uFEFFmple.com/x",
    "https://exa\u0085mple.com/x",
    "<U+202E>https://example.com/x",
  ];
  const policy = targets.flatMap((uri) =>
    labels.flatMap((label) =>
      [label, ` \t${label}\uFEFF`].map((text) => ({
        text,
        uri,
        mismatch: api.linkTextMismatch(text, uri),
      }))
    )
  );
  let unsafe = "<U+202E> literal <U+034F> ❤\uFE0F ";
  for (let point = 0; point <= 0x10FFFF; point++) {
    const ch = String.fromCodePoint(point);
    if (/[\p{Cc}\p{Cf}\u034F\u115F\u1160\u17B4-\u17B5\u2028\u2029\u3164\uFFA0\u{E0000}-\u{E007F}]/u.test(ch)) {
      unsafe += ch;
    }
  }
  const formats = [unsafe, "a".repeat(139) + "😀".repeat(200), `https://${"a".repeat(5000)}@evil.example/path`].map((
    text,
  ) => ({
    text,
    visible: api.visibleFormatCharacters(text),
    shortened: nativeText(api.shortenedTarget(text)),
  }));
  const targetCases = targets.map((uri) => ({
    uri,
    displayed: api.displayedTarget(uri),
    plain: api.isPlainWebUrl(uri),
    host: api.linkTargetParts(uri).find((part: { host: boolean }) => part.host)?.text ?? null,
  }));

  // Rows use actual wire tuples. The minimal source adapter supplies decoded
  // rows to the shipping detector and hover reader without any DOM emulation.
  const target = "https://example.com/x";
  const osc = ["tag", target];
  const row = (text: string, wrapped = false, index?: number) => ({
    w: wrapped,
    r: [[0, text.length, text, [0, -1, -1], ...(index === undefined ? [] : [index])]],
  });
  const inputs = [
    { cols: 40, rows: [row(target)], links: [] },
    { cols: 14, rows: [row("https://exampl", true), row("e.com/x")], links: [] },
    { cols: 40, rows: [row(target, false, 0)], links: [osc] },
    { cols: 40, rows: [row("https://example.com", false, 0), row(".evil.test/x", false, 0)], links: [osc] },
    { cols: 40, rows: [row(target, false, 0), row(target, false, 0)], links: [osc] },
    { cols: 40, rows: [row("click here", false, 0)], links: [osc] },
    { cols: 40, rows: [row("https://exa mple.com/x", false, 0)], links: [osc] },
    { cols: 40, rows: [row(target, false, 0)], links: [["tag", "file:///tmp/x"]] },
    {
      cols: 8,
      rows: [row("https://", true, 0), row("example.", true, 1), row("com/x", false, 2)],
      links: [["1_alacritty", target], ["2_alacritty", target], ["3_alacritty", target]],
    },
    {
      cols: 40,
      rows: [
        { w: false, r: [[0, 5, "https", [0, -1, -1], 0], [8, 4, "more", [0, -1, -1], 0]] },
        row(".evil.test/x", false, 0),
      ],
      links: [osc],
    },
    { cols: 40, rows: Array.from({ length: 128 }, () => row(target, false, 0)), links: [osc] },
    { cols: 40, rows: Array.from({ length: 129 }, () => row(target, false, 0)), links: [osc] },
    { cols: 100, rows: Array.from({ length: 100 }, () => row("a".repeat(90), false, 0)), links: [osc] },
    { cols: 40, rows: [row("https://github.com/<U+202E>", false, 0)], links: [osc] },
    {
      cols: 40,
      rows: [{ w: false, r: [[0, 1, "\u2800", [0, -1, -1], 0], [1, 23, "https://github.com/test", [0, -1, -1], 0]] }],
      links: [osc],
    },
  ];
  for (const ids of [["tag", "tag"], ["1_alacritty", "2_alacritty"]]) {
    inputs.push({
      cols: 16,
      rows: [row("https://a.test/", true, 0), { w: false, r: [[0, 2, "😀", [0, -1, -1], 1]] }],
      links: ids.map((id) => [id, "https://a.test/😀"]),
    });
  }
  for (const text of labels.filter((text) => /^[\x20-\x7e]+$/.test(text))) {
    for (const suffix of ["", ".", ")", "!", "?", "]", " rest"]) {
      inputs.push({ cols: 200, rows: [row(text + suffix)], links: [] });
    }
  }
  const grids = inputs.map((input) => {
    const rows = input.rows.map((wire) => {
      const runs = wire.r.map((run) => ({
        col: run[0] as number,
        width: run[1] as number,
        text: run[2] as string,
        link: run.length === 5 ? { id: input.links[run[4] as number][0], uri: input.links[run[4] as number][1] } : null,
      }));
      return { width: input.cols, row: { wrapped: wire.w, end: Math.max(...runs.map((r) => r.col + r.width)), runs } };
    });
    const source = { line: (at: number) => rows[at] ?? null };
    const questions = [...new Set([0, Math.floor(rows.length / 2), rows.length - 1])].map((line) => ({
      line,
      expected: buffer.rowLinks(source, line).map((link: any) => {
        const parsed = (() => {
          try {
            return new URL(link.uri);
          } catch {
            return null;
          }
        })();
        const web = parsed && ["http:", "https:"].includes(parsed.protocol) && parsed.host &&
          (link.kind === "osc" || api.isPlainWebUrl(link.uri));
        const label = link.kind === "osc"
          ? api.linkHoverText(buffer.linkLines(source), link.ranges[0], link.uri)
          : null;
        const warning = label?.kind === "fragment"
          ? api.linkTextMismatch(label.text, "not a url")
          : api.linkTextMismatch(label?.text, link.uri);
        return {
          ...link,
          evidence: link.evidence.filter((at: number) => at >= 0),
          disclosure: web
            ? {
              host: parsed!.host,
              target: nativeText(api.shortenedTarget(api.displayedTarget(link.uri))),
              claimed: warning ? nativeText(api.shortenedTarget(api.visibleFormatCharacters(label.text.trim()))) : null,
            }
            : null,
        };
      }),
    }));
    return { ...input, questions };
  });
  const output = new URL("../tests/fixtures/links-oracle.json", import.meta.url);
  await Deno.mkdir(new URL(".", output), { recursive: true });
  await Deno.writeTextFile(
    output,
    JSON.stringify({ sources: Object.fromEntries(sources), policy, formats, targets: targetCases, grids }) + "\n",
  );
  console.log(
    `${policy.length} comparisons, ${formats.length} format cases, ${grids.length} grids -> ${output.pathname}`,
  );
}
