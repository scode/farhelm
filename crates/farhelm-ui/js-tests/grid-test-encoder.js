// Test-only wire producer for the grid model and browser component fixtures.
//
// Production has one producer, Rust's GridOutput::encode. Tests keep readable
// message objects, but must feed the real binary boundary to the client. The
// cross-language fixture suite holds this small writer byte-equal to Rust;
// it is never served by the product or used by the performance benchmark.
(function (root) {
  "use strict";

  const utf8 = new TextEncoder();

  /** A fixture writer, bounded by each field's actual representation.
   * It does not validate the model's semantics: callers may deliberately
   * encode an out-of-row column or an invalid color for refusal tests.
   */
  class Writer {
    constructor() {
      this.bytes = [];
    }

    integer(value, maximum) {
      if (!Number.isSafeInteger(value) || value < 0 || value > maximum) throw new Error("unrepresentable test integer");
    }

    u8(value) {
      this.integer(value, 0xff);
      this.bytes.push(value);
    }
    u16(value) {
      this.integer(value, 0xffff);
      this.bytes.push(value & 255, value >>> 8);
    }
    u32(value) {
      this.integer(value, 0xffffffff);
      this.bytes.push(value & 255, value >>> 8 & 255, value >>> 16 & 255, value >>> 24);
    }

    i32(value) {
      if (!Number.isInteger(value) || value < -0x80000000 || value > 0x7fffffff) {
        throw new Error("unrepresentable test color");
      }
      this.u32(value >>> 0);
    }

    counter(value) {
      this.integer(value, Number.MAX_SAFE_INTEGER);
      this.u32(value % 0x100000000);
      this.u32(Math.floor(value / 0x100000000));
    }

    raw(bytes) {
      for (const byte of bytes) this.bytes.push(byte);
    }
    finish() {
      return Uint8Array.from(this.bytes);
    }
  }

  /** Preserve the existing message-local hyperlink table without sharing state between messages. */
  function links(writer, values) {
    writer.u16(values.length);
    for (const [id, uri] of values) {
      for (const text of [id, uri]) {
        const bytes = utf8.encode(text);
        writer.u16(bytes.length);
        writer.raw(bytes);
      }
    }
  }

  /** One independently length-delimited row, including its optional style fields. */
  function row(writer, value) {
    const body = new Writer();
    if (typeof value.w !== "boolean") throw new Error("bad test wrap flag");
    body.u8(Number(value.w));
    body.u16(value.r.length);
    for (const run of value.r) {
      const [col, width, text, style] = run;
      const [flags, fg, bg] = style;
      if (!Number.isInteger(flags) || flags < 0 || flags > 0xfff) throw new Error("unknown test style bits");
      const bytes = utf8.encode(text);
      const hasLink = run.length === 5;
      const meta = flags | (fg !== -1 ? 0x1000 : 0) | (bg !== -1 ? 0x2000 : 0) |
        (style.length === 4 ? 0x4000 : 0) | (hasLink ? 0x8000 : 0);
      body.u16(col);
      body.u16(width);
      body.u16(meta);
      body.u16(bytes.length);
      if (fg !== -1) body.i32(fg);
      if (bg !== -1) body.i32(bg);
      if (style.length === 4) body.i32(style[3]);
      if (hasLink) body.u16(run[4]);
      body.raw(bytes);
    }
    writer.u32(body.bytes.length);
    writer.raw(body.bytes);
  }

  /** Encode test output exactly as the current Rust producer does, leaving controls JSON. */
  function encode(value) {
    let message = value;
    if (typeof value === "string") {
      try {
        message = JSON.parse(value);
      } catch (_error) {
        return utf8.encode(value);
      }
    }
    const kind = message && (message.t === "rows" ? 1 : message.t === "frame" ? 2 : message.t === "history" ? 3 : 0);
    if (!kind) return utf8.encode(typeof value === "string" ? value : JSON.stringify(message));
    const writer = new Writer();
    writer.u8(0);
    writer.u8(kind);
    if (kind === 2) {
      writer.counter(message.rev);
      for (const value of [message.cols, message.rows, message.shift || 0, message.cursor.x, message.cursor.y]) {
        writer.u16(value);
      }
      const shape = ["block", "underline", "bar"].indexOf(message.cursor.shape);
      if (shape < 0) throw new Error("bad test cursor shape");
      writer.u8(Number(message.cursor.visible) | Number(message.cursor.blink) << 1 | shape << 2);
    } else {
      if (kind === 3) {
        writer.counter(message.first);
        writer.u16(message.cols);
      }
      links(writer, message.links || []);
      writer.u32(message.rows.length);
      for (const entry of message.rows) {
        if (kind === 1) {
          writer.u16(entry[0]);
          row(writer, entry[1]);
        } else row(writer, entry);
      }
    }
    return writer.finish();
  }

  const api = { encode };
  root.farhelmGridTestEncoder = api;
  if (typeof module === "object" && module.exports) module.exports = api;
})(typeof window === "object" ? window : globalThis);
