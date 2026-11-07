// What a program reads from the input frames a page sends on its terminal
// sockets, for the specs that prove a shortcut or a dialog kept keystrokes
// away from the program. Kept free of Playwright's runtime so that its
// decoding has unit tests of its own (program-input.test.ts, run with
// `node --test` in e2e/harness-tests' way).
//
// Which protocol a frame speaks is the attachment's, never guessed from the
// bytes: a socket's kind is the helm's stream notice on it (`{"type":
// "stream","kind":...}`, terminal.js's `noteStreamKind`), and a socket that
// sent no notice speaks bytes, as terminal.js reads it.

/** One socket's stream kind as its notice set it; null until a notice arrives. */
export interface SocketKind {
  kind: "bytes" | "grid" | null;
}

/** One binary frame a page sent, with its socket's stream kind when it went out. */
export interface InputFrame {
  payload: Buffer;
  socket: SocketKind;
}

/** The parts of Playwright's WebSocket this needs, so tests can pass a stand-in. */
interface FrameSource {
  on(event: "framesent" | "framereceived", listener: (frame: { payload: string | Buffer }) => void): unknown;
}

/**
 * Collect every binary frame `socket` sends into `frames`, tagged with the
 * socket's stream kind (learned from the notice it receives), for
 * `programInput`. Callers keep `frames` as their own array and may empty it
 * (`frames.length = 0`) to start a new observation.
 */
export function recordInput(socket: FrameSource, frames: InputFrame[]): void {
  const state: SocketKind = { kind: null };
  socket.on("framereceived", ({ payload }) => {
    if (typeof payload !== "string") return;
    try {
      const message = JSON.parse(payload);
      if (message && message.type === "stream" && (message.kind === "bytes" || message.kind === "grid")) {
        state.kind = message.kind;
      }
    } catch {
      // Not a control message this needs.
    }
  });
  socket.on("framesent", ({ payload }) => {
    // The kind as it stands when the frame goes out (the notice precedes any
    // input, so it is already known then).
    if (Buffer.isBuffer(payload)) frames.push({ payload, socket: { kind: state.kind } });
  });
}

/**
 * The bytes the supervisor writes for the named keys these specs send, keyed
 * by `mods:kc` (Shift is 1), with the `key` the client sends beside them: the
 * supervisor chooses a named key's bytes by its numeric code, not its name
 * (crates/farhelm-supervisor/src/grid_input.rs, `key_sequence`), and Farhelm's
 * key table (crates/farhelm-supervisor/tests/fixtures/grid-keys.jsonl) is the
 * authority. Only combinations whose bytes are fixed whatever the program's
 * modes are here.
 */
const NAMED_KEYS: Record<string, { key: string; bytes: string }> = {
  "0:13": { key: "Enter", bytes: "\r" },
  "1:13": { key: "Enter", bytes: "\x1b\r" },
  "0:9": { key: "Tab", bytes: "\t" },
  "1:9": { key: "Tab", bytes: "\x1b[Z" },
  "0:8": { key: "Backspace", bytes: "\x7f" },
  "0:27": { key: "Escape", bytes: "\x1b" },
};

/** The numeric codes `key_sequence` gives a meaning of their own above 47 (F1 to F12). */
const FUNCTION_KEY_CODES = { first: 112, last: 123 };

/**
 * One grid key message as the program reads it, or null when the decoder
 * cannot vouch for the bytes. Accepted, as the supervisor encodes them:
 * the named keys above when `key` agrees with `kc`; Ctrl alone with a letter
 * (`kc` 65 to 90, `key` that letter), its control character; and a key that
 * types one character (no modifier but Shift, `kc` 48 or more outside the
 * function keys, `key` one UTF-16 unit), that character.
 */
function keyMessageInput(message: { key: string; kc: number; mods: number }): string | null {
  const id = `${message.mods}:${message.kc}`;
  if (Object.hasOwn(NAMED_KEYS, id)) return message.key === NAMED_KEYS[id].key ? NAMED_KEYS[id].bytes : null;
  if (message.mods === 4 && message.kc >= 65 && message.kc <= 90) {
    return message.key.length === 1 && message.key.toUpperCase().charCodeAt(0) === message.kc
      ? String.fromCharCode(message.kc - 64)
      : null;
  }
  const typesOne = (message.mods === 0 || message.mods === 1) && message.kc >= 48 &&
    !(message.kc >= FUNCTION_KEY_CODES.first && message.kc <= FUNCTION_KEY_CODES.last) && message.key.length === 1;
  return typesOne ? message.key : null;
}

/** One grid input message as the program reads it, or a `‹…›` token naming what it is. */
function gridMessageInput(payload: Buffer): string {
  let message: any;
  try {
    message = JSON.parse(payload.toString());
  } catch {
    return "‹unreadable›";
  }
  if (!message || typeof message !== "object") return "‹unreadable›";
  if (message.t === "text" && typeof message.data === "string") return message.data;
  if (message.t === "key") {
    // The fields the grid format requires of a key (crates/farhelm-proto's
    // `GridInput::Key`): a message missing one is refused whole by the
    // supervisor, so the program reads nothing from it.
    const wellFormed = typeof message.key === "string" && typeof message.code === "string" &&
      Number.isInteger(message.kc) && Number.isInteger(message.mods) &&
      (message.mac === undefined || typeof message.mac === "boolean");
    if (!wellFormed) return "‹key malformed›";
    return keyMessageInput(message) ?? `‹key ${message.mods}:${message.kc}:${message.key}›`;
  }
  return `‹${typeof message.t === "string" ? message.t : "unreadable"}›`;
}

/**
 * What the programs read from `frames`, in order: a byte stream's frames as
 * the bytes they are (consecutive ones decoded together, as the specs always
 * read them), a grid stream's input messages the way the supervisor encodes
 * the ones these specs produce (`keyMessageInput`). Anything else in a
 * grid stream reads as a `‹…›` token, which no typed text contains, so an
 * unexpected message fails an exact comparison.
 */
export function programInput(frames: InputFrame[]): string {
  let out = "";
  let bytes: Buffer[] = [];
  const flush = () => {
    if (bytes.length) out += Buffer.concat(bytes).toString();
    bytes = [];
  };
  for (const frame of frames) {
    if (frame.socket.kind === "grid") {
      flush();
      out += gridMessageInput(frame.payload);
    } else {
      bytes.push(frame.payload);
    }
  }
  flush();
  return out;
}
