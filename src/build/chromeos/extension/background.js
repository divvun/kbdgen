// The input method's service worker (docs/spec/ldml/chromeos.md): it runs
// kbd-engine, compiled to WebAssembly as engine.wasm, on the model of the
// active layout, models/<id>.dvkb, and applies the engine's edits through
// chrome.input.ime. It never logs typed text or keys.
//
// Listeners are registered synchronously on the first evaluation, as a
// Manifest V3 service worker must. A key that arrives before the engine
// and model have loaded is answered later with keyEventHandled; until the
// backlog drains, every key goes through that queue so that keys apply in
// order.
"use strict";

// The PC/AT set 1 scan code, without E0, of each KeyboardEvent.code that is
// sent to the engine as a hardware key. Extended keys, such as the arrows,
// Delete and Right Ctrl, have none: they are frame keys and pass.
const SCAN_CODES = new Map([
  ["Escape", 0x01],
  ["Digit1", 0x02],
  ["Digit2", 0x03],
  ["Digit3", 0x04],
  ["Digit4", 0x05],
  ["Digit5", 0x06],
  ["Digit6", 0x07],
  ["Digit7", 0x08],
  ["Digit8", 0x09],
  ["Digit9", 0x0a],
  ["Digit0", 0x0b],
  ["Minus", 0x0c],
  ["Equal", 0x0d],
  ["Tab", 0x0f],
  ["KeyQ", 0x10],
  ["KeyW", 0x11],
  ["KeyE", 0x12],
  ["KeyR", 0x13],
  ["KeyT", 0x14],
  ["KeyY", 0x15],
  ["KeyU", 0x16],
  ["KeyI", 0x17],
  ["KeyO", 0x18],
  ["KeyP", 0x19],
  ["BracketLeft", 0x1a],
  ["BracketRight", 0x1b],
  ["Enter", 0x1c],
  ["KeyA", 0x1e],
  ["KeyS", 0x1f],
  ["KeyD", 0x20],
  ["KeyF", 0x21],
  ["KeyG", 0x22],
  ["KeyH", 0x23],
  ["KeyJ", 0x24],
  ["KeyK", 0x25],
  ["KeyL", 0x26],
  ["Semicolon", 0x27],
  ["Quote", 0x28],
  ["Backquote", 0x29],
  ["Backslash", 0x2b],
  ["KeyZ", 0x2c],
  ["KeyX", 0x2d],
  ["KeyC", 0x2e],
  ["KeyV", 0x2f],
  ["KeyB", 0x30],
  ["KeyN", 0x31],
  ["KeyM", 0x32],
  ["Comma", 0x33],
  ["Period", 0x34],
  ["Slash", 0x35],
  ["NumpadMultiply", 0x37],
  ["Space", 0x39],
  ["F1", 0x3b],
  ["F2", 0x3c],
  ["F3", 0x3d],
  ["F4", 0x3e],
  ["F5", 0x3f],
  ["F6", 0x40],
  ["F7", 0x41],
  ["F8", 0x42],
  ["F9", 0x43],
  ["F10", 0x44],
  ["ScrollLock", 0x46],
  ["Numpad7", 0x47],
  ["Numpad8", 0x48],
  ["Numpad9", 0x49],
  ["NumpadSubtract", 0x4a],
  ["Numpad4", 0x4b],
  ["Numpad5", 0x4c],
  ["Numpad6", 0x4d],
  ["NumpadAdd", 0x4e],
  ["Numpad1", 0x4f],
  ["Numpad2", 0x50],
  ["Numpad3", 0x51],
  ["Numpad0", 0x52],
  ["IntlBackslash", 0x56],
  ["F11", 0x57],
  ["F12", 0x58],
  ["NumpadEqual", 0x59],
  ["KanaMode", 0x70],
  ["Lang2", 0x71],
  ["Lang1", 0x72],
  ["IntlRo", 0x73],
  ["Convert", 0x79],
  ["NonConvert", 0x7b],
  ["IntlYen", 0x7d],
  ["NumpadComma", 0x7e],
]);

// Keys that pass without resetting anything. ChromeOS calls the Search key
// Meta; older Chrome called it OS.
const MODIFIER_KEYS = new Set([
  "ShiftLeft",
  "ShiftRight",
  "ControlLeft",
  "ControlRight",
  "AltLeft",
  "AltRight",
  "MetaLeft",
  "MetaRight",
  "OSLeft",
  "OSRight",
  "CapsLock",
]);

// kbd_key's keys, modifier bits, flags and results (crates/kbd-wasm).
const KEY_DECIMAL = 0x100;
const KEY_BACKSPACE = 0x101;
const KEY_COMMIT = 0x102;
const SHIFT_L = 1;
const SHIFT_R = 1 << 1;
const CAPS = 1 << 2;
const CTRL_L = 1 << 3;
const CTRL_R = 1 << 4;
const ALT_L = 1 << 5;
const ALT_R = 1 << 6;
const ALTGR = 1 << 7;
const CMD = 1 << 8;
const EXTRA1 = 1 << 9;
const AT_START = 1;
const AUTHORITATIVE = 1 << 1;
const PASS = 0;
const EDIT = 1;
// kbd_extra_modifiers' key codes.
const EXTRA_CAPS_LOCK = 2;
const EXTRA_B00 = 3;

const MODIFIER = -1;
const OTHER = -2;

// The scalar values kept before the caret: more than any model's
// context_len, which is at most 64.
const MAX_CONTEXT = 128;
// The most expected states kept while surrounding-text notifications lag.
const MAX_SNAPSHOTS = 64;

const ime = chrome.input.ime;
const encoder = new TextEncoder();
const decoder = new TextDecoder();

class Engine {
  constructor(exports) {
    this.x = exports;
  }

  // A fresh view each time: any call may grow the memory.
  memory() {
    return new Uint8Array(this.x.memory.buffer);
  }

  write(bytes) {
    const at = this.x.kbd_input(bytes.length);
    this.memory().set(bytes, at);
    return bytes.length;
  }

  output(from, to) {
    const at = this.x.kbd_output();
    return decoder.decode(this.memory().subarray(at + from, at + to));
  }

  load(bytes) {
    const handle = this.x.kbd_load(this.write(bytes));
    if (handle === 0) {
      throw new Error(this.output(0, this.x.kbd_output_len()));
    }
    return handle;
  }

  // The edit for a key, or null when the key passes.
  key(handle, key, modifiers, text, flags) {
    const length = this.write(encoder.encode(text));
    const result = this.x.kbd_key(handle, key, modifiers, length, flags);
    if (result === PASS) {
      return null;
    }
    if (result !== EDIT) {
      throw new Error(`kbd_key failed with ${result}`);
    }
    const split = this.x.kbd_insert_len();
    return {
      delete: this.x.kbd_delete(),
      insert: this.output(0, split),
      preedit: this.output(split, this.x.kbd_output_len()),
    };
  }
}

async function packaged(path) {
  const response = await fetch(chrome.runtime.getURL(path));
  if (!response.ok) {
    throw new Error(`${path}: ${response.status}`);
  }
  return new Uint8Array(await response.arrayBuffer());
}

let engine = null;
// Layout id to the promise of its session, and to the session once loaded.
const loading = new Map();
const sessions = new Map();

function loadEngine() {
  engine ??= packaged("engine.wasm")
    .then((bytes) => WebAssembly.instantiate(bytes, {}))
    .then(({ instance }) => new Engine(instance.exports));
  return engine;
}

function loadSession(id) {
  let session = loading.get(id);
  if (session === undefined) {
    session = Promise.all([loadEngine(), packaged(`models/${id}.dvkb`)]).then(
      ([loadedEngine, bytes]) => {
        const handle = loadedEngine.load(bytes);
        const extras = loadedEngine.x.kbd_extra_modifiers(handle);
        const loaded = {
          id,
          engine: loadedEngine,
          handle,
          extras: [0, 1, 2].map((slot) => (extras >> (2 * slot)) & 3),
          shiftLock: loadedEngine.x.kbd_shift_lock(handle) !== 0,
        };
        sessions.set(id, loaded);
        return loaded;
      },
    );
    session.catch((error) => {
      console.error(`Divvun keyboard ${id} could not load: ${error.message}`);
    });
    loading.set(id, session);
  }
  return session;
}

// The physical keys held, as key events have shown them, and the Shift
// Lock state of a keyboard with shiftLock: Caps Lock sets it, Shift clears
// it.
const held = new Set();
let shiftLock = false;

// What is known about the focused text field. `text` is the committed text
// before the caret, as far as notifications and the engine's own edits
// tell; `composing` is the preedit shown as the composition; `snapshots`
// are the states the engine's own operations lead to, oldest first, which
// surrounding-text notifications may still report after the fact.
const field = {
  id: null,
  password: false,
  text: "",
  authoritative: false,
  atStart: false,
  selection: false,
  composing: "",
  snapshots: [],
};

function current() {
  return { text: field.text, composing: field.composing };
}

function expect() {
  const last = field.snapshots[field.snapshots.length - 1];
  if (last?.text === field.text && last?.composing === field.composing) {
    return;
  }
  field.snapshots.push(current());
  if (field.snapshots.length > MAX_SNAPSHOTS) {
    field.snapshots.shift();
  }
}

// Forgets the text before the caret, as after the caret moved.
function forget() {
  field.text = "";
  field.authoritative = false;
  field.atStart = false;
  field.selection = false;
  field.composing = "";
  field.snapshots = [current()];
}

function resetSessions() {
  for (const session of sessions.values()) {
    session.engine.x.kbd_reset(session.handle);
  }
}

function releaseAll() {
  held.clear();
  resetSessions();
  forget();
}

function trim(text) {
  const scalars = Array.from(text);
  if (scalars.length <= MAX_CONTEXT) {
    return text;
  }
  field.atStart = false;
  return scalars.slice(scalars.length - MAX_CONTEXT).join("");
}

// The UTF-16 units of the last `scalars` scalar values of `text`, which
// is what deleteSurroundingText counts. A lone surrogate is one scalar,
// as TextEncoder makes it U+FFFD for the engine.
function utf16Units(text, scalars) {
  let units = 0;
  let i = text.length;
  for (; scalars > 0 && i > 0; scalars--) {
    const low = text.charCodeAt(i - 1);
    const high = i >= 2 ? text.charCodeAt(i - 2) : 0;
    const pair = low >= 0xdc00 && low <= 0xdfff && high >= 0xd800 && high <= 0xdbff;
    const n = pair ? 2 : 1;
    units += n;
    i -= n;
  }
  return units;
}

function contextText() {
  return field.password ? "" : field.text;
}

function contextFlags() {
  if (field.password) {
    return 0;
  }
  return (field.atStart ? AT_START : 0) | (field.authoritative ? AUTHORITATIVE : 0);
}

// A failed IME call leaves the field in a state the glue cannot know.
function checked(result, session) {
  result?.catch?.(() => {
    session.engine.x.kbd_reset(session.handle);
    forget();
  });
}

// Applies an edit: the composition is cleared when text changes under it,
// then `delete` scalar values before the caret go, `insert` is committed
// and the preedit is shown as the new composition. A password field shows
// no preedit and keeps no text.
function apply(session, edit) {
  const contextID = field.id;
  const shown = field.password ? "" : edit.preedit;
  const units = utf16Units(contextText(), edit.delete);
  if (field.composing !== "" && (units > 0 || edit.insert !== "" || shown === "")) {
    checked(ime.clearComposition({ contextID }), session);
    field.composing = "";
    expect();
  }
  if (units > 0) {
    checked(
      ime.deleteSurroundingText({ engineID: session.id, contextID, offset: -units, length: units }),
      session,
    );
    field.text = field.text.slice(0, field.text.length - units);
    expect();
  }
  if (edit.insert !== "") {
    checked(ime.commitText({ contextID, text: edit.insert }), session);
    if (!field.password) {
      field.text = trim(field.text + edit.insert);
    }
    expect();
  }
  if (shown !== field.composing) {
    checked(ime.setComposition({ contextID, text: shown, cursor: shown.length }), session);
    field.composing = shown;
    expect();
  }
}

// A key other than a lone modifier passes: what is pending is committed,
// then the engine resets. The text before the caret is kept, since a
// notification follows if the application changes it.
function pass(session) {
  if (field.composing !== "" || session.engine.x.kbd_pending(session.handle) > 0) {
    const edit = session.engine.key(
      session.handle,
      KEY_COMMIT,
      0,
      contextText(),
      contextFlags(),
    );
    if (edit !== null) {
      apply(session, edit);
    }
  }
  session.engine.x.kbd_reset(session.handle);
  field.snapshots = [current()];
}

function role(code, key) {
  if (MODIFIER_KEYS.has(code)) {
    return MODIFIER;
  }
  if (code === "Backspace") {
    return KEY_BACKSPACE;
  }
  if (code === "NumpadDecimal") {
    // With Num Lock off the key is Delete.
    return key === "Delete" ? OTHER : KEY_DECIMAL;
  }
  return SCAN_CODES.get(code) ?? OTHER;
}

function observe(keyData) {
  const code = keyData.code;
  if (keyData.type === "keydown") {
    held.add(code);
    if (code === "CapsLock") {
      shiftLock = true;
    } else if (code === "ShiftLeft" || code === "ShiftRight") {
      shiftLock = false;
    }
  } else {
    held.delete(code);
  }
}

// The key down as the engine will see it, captured when it arrives: the
// flags say which modifiers are down, the held keys which side.
function capture(keyData) {
  return {
    role: role(keyData.code, keyData.key),
    shift: keyData.shiftKey === true,
    ctrl: keyData.ctrlKey === true,
    alt: keyData.altKey === true,
    altgr: keyData.altgrKey === true,
    caps: keyData.capsLock === true,
    shiftLock,
    held: new Set(held),
  };
}

function sides(down, held, left, right) {
  if (!down) {
    return 0;
  }
  const r = held.has(right);
  return (held.has(left) || !r ? 1 : 0) | (r ? 2 : 0);
}

// Right Alt is AltGr whether the XKB layout beneath reports it as AltGr or
// as Alt; Left Alt is Alt, which the engine passes on ChromeOS.
function modifierBits(e, session) {
  let bits = 0;
  const shift = sides(e.shift, e.held, "ShiftLeft", "ShiftRight");
  const ctrl = sides(e.ctrl, e.held, "ControlLeft", "ControlRight");
  if (shift & 1) bits |= SHIFT_L;
  if (shift & 2) bits |= SHIFT_R;
  if (ctrl & 1) bits |= CTRL_L;
  if (ctrl & 2) bits |= CTRL_R;
  const altRight = e.altgr || (e.alt && e.held.has("AltRight"));
  const altLeft = e.alt && (e.held.has("AltLeft") || !e.held.has("AltRight"));
  if (altLeft) bits |= ALT_L;
  if (altRight) bits |= ALT_R | ALTGR;
  if (session.shiftLock ? e.shiftLock : e.caps) bits |= CAPS;
  if (["MetaLeft", "MetaRight", "OSLeft", "OSRight"].some((k) => e.held.has(k))) {
    bits |= CMD;
  }
  session.extras.forEach((kind, slot) => {
    const down =
      (kind === EXTRA_CAPS_LOCK && e.held.has("CapsLock")) ||
      (kind === EXTRA_B00 && e.held.has("IntlBackslash"));
    if (down) bits |= EXTRA1 << slot;
  });
  return bits;
}

// Whether the key is handled; false lets it through to the application.
function handle(session, e) {
  if (field.id === null || e.role === MODIFIER) {
    return false;
  }
  try {
    if (e.role === OTHER) {
      pass(session);
      return false;
    }
    if (field.selection) {
      // The typed text replaces the selection, so nothing before the caret
      // is known to stay.
      session.engine.x.kbd_reset(session.handle);
      forget();
    }
    const edit = session.engine.key(
      session.handle,
      e.role,
      modifierBits(e, session),
      contextText(),
      contextFlags(),
    );
    if (edit === null) {
      pass(session);
      return false;
    }
    apply(session, edit);
    return true;
  } catch (error) {
    console.error(`Divvun keyboard ${session.id}: ${error.message}`);
    session.engine.x.kbd_reset(session.handle);
    forget();
    return false;
  }
}

let backlog = Promise.resolve();
let waiting = 0;

ime.onKeyEvent.addListener((engineID, keyData, requestId) => {
  observe(keyData);
  if (keyData.type !== "keydown") {
    return false;
  }
  const e = capture(keyData);
  const session = sessions.get(engineID);
  if (waiting === 0 && session !== undefined) {
    return handle(session, e);
  }
  waiting += 1;
  backlog = backlog.then(async () => {
    let handled = false;
    try {
      handled = handle(await loadSession(engineID), e);
    } catch {
      handled = false;
    }
    waiting -= 1;
    ime.keyEventHandled(requestId, handled);
  });
  return undefined;
});

// Whether a reading of the text before the caret agrees with the text
// expected there. A reading that starts at the start of the field holds all
// of it, so it ends with what is expected; one cut at 100 characters may
// also be the end of a longer expected text.
function agrees(reading, whole, expected) {
  return reading.endsWith(expected) || (!whole && expected.endsWith(reading));
}

// The committed text a reading shows, if it is of `snapshot`. Whether a
// reading includes the composition is Chrome's choice, so both are tried.
function committed(reading, whole, snapshot) {
  const composing = snapshot.composing;
  if (
    composing !== "" &&
    reading.endsWith(composing) &&
    agrees(reading, whole, snapshot.text + composing)
  ) {
    return reading.slice(0, reading.length - composing.length);
  }
  if (agrees(reading, whole, snapshot.text)) {
    return reading;
  }
  return null;
}

function adopt(text, atStart) {
  if (atStart || Array.from(text).length >= Array.from(field.text).length) {
    field.text = trim(text);
    field.atStart = atStart;
  }
  field.authoritative = true;
  field.snapshots = [current()];
}

// A reading of an older state than the latest edit's lags behind it and is
// dropped; one of the latest state completes what the glue knows; any
// other means the text changed by other means and replaces it. The engine
// state survives: the engine drops markers whose text is gone
// (ldml.engine.context).
function reconcile(reading, atStart) {
  const snapshots = field.snapshots;
  for (let i = snapshots.length - 1; i >= 0; i--) {
    const text = committed(reading, atStart, snapshots[i]);
    if (text === null) {
      continue;
    }
    if (i === snapshots.length - 1) {
      adopt(text, atStart);
    } else {
      snapshots.splice(0, i);
    }
    return;
  }
  const composing = field.composing;
  const text =
    composing !== "" && reading.endsWith(composing)
      ? reading.slice(0, reading.length - composing.length)
      : reading;
  field.text = trim(text);
  field.atStart = atStart;
  field.authoritative = true;
  field.snapshots = [current()];
}

ime.onSurroundingTextChanged.addListener((_engineID, info) => {
  if (field.id === null || field.password) {
    return;
  }
  field.selection = info.anchor !== info.focus;
  if (field.selection) {
    return;
  }
  reconcile(info.text.slice(0, info.focus), info.offset === 0);
});

ime.onFocus.addListener((context) => {
  releaseAll();
  field.id = context.contextID;
  field.password = context.type === "password";
});

ime.onBlur.addListener((contextID) => {
  if (field.id === contextID) {
    field.id = null;
  }
  releaseAll();
});

// Chrome ended the input session, for instance when the caret was clicked
// elsewhere, and settled the composition itself.
ime.onReset.addListener(() => {
  resetSessions();
  forget();
});

ime.onActivate.addListener((engineID) => {
  releaseAll();
  loadSession(engineID).catch(() => {});
});

ime.onDeactivated.addListener(() => {
  releaseAll();
});
