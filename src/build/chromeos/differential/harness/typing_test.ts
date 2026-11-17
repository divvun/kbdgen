// The harness of ldml.chromeos.test.typing. `KBDGEN_CHROMEOS_CASES` names
// the cases file the Rust test writes: extensions kbdgen generated, and key
// event sequences with what kbd-engine types natively for them. Each
// extension's own background.js runs here against a fake chrome.input.ime
// whose text field applies the IME calls and reports the text before the
// caret, as ChromeOS does, after the fact.

import { pathToFileURL } from "node:url";

interface KeyData {
  type: "keydown" | "keyup";
  code: string;
  key: string;
  shiftKey: boolean;
  ctrlKey: boolean;
  altKey: boolean;
  altgrKey: boolean;
  capsLock: boolean;
}

interface Case {
  label: string;
  events: KeyData[];
  handled: boolean[];
  text: string;
  preedit: string;
}

interface Fixture {
  label: string;
  extension: string;
  engine: string;
  cases: Case[];
}

interface SurroundingInfo {
  text: string;
  focus: number;
  anchor: number;
  offset: number;
}

// Whether a reading of the text before the caret includes the composition,
// and whether readings arrive one key late; ChromeOS gives no guarantee of
// either.
interface Mode {
  name: string;
  includeComposition: boolean;
  lag: boolean;
}

const MODES: Mode[] = [
  { name: "composition shown, on time", includeComposition: true, lag: false },
  { name: "composition hidden, one key late", includeComposition: false, lag: true },
];

// ChromeOS reports at most 100 characters before the caret.
const READING_LIMIT = 100;

class Listeners<A extends unknown[], R = void> {
  #listeners: Array<(...args: A) => R> = [];

  addListener(listener: (...args: A) => R): void {
    this.#listeners.push(listener);
  }

  fire(...args: A): R[] {
    return this.#listeners.map((listener) => listener(...args));
  }
}

class FakeChrome {
  before = "";
  composition = "";
  contextID = 0;
  requests = 0;
  readings: SurroundingInfo[] = [];
  late: SurroundingInfo[] = [];
  answers = new Map<string, boolean>();
  problems: string[] = [];

  readonly ime = {
    onKeyEvent: new Listeners<[string, KeyData, string], boolean | undefined>(),
    onSurroundingTextChanged: new Listeners<[string, SurroundingInfo]>(),
    onFocus: new Listeners<[{ contextID: number; type: string }]>(),
    onBlur: new Listeners<[number]>(),
    onReset: new Listeners<[string]>(),
    onActivate: new Listeners<[string, string]>(),
    onDeactivated: new Listeners<[string]>(),
    commitText: (p: { contextID: number; text: string }) => {
      this.check(p.contextID, "commitText");
      this.composition = "";
      this.before += p.text;
      return this.changed();
    },
    deleteSurroundingText: (
      p: { engineID: string; contextID: number; offset: number; length: number },
    ) => {
      this.check(p.contextID, "deleteSurroundingText");
      if (p.engineID !== this.engine) {
        this.problems.push(`deleteSurroundingText for engine ${p.engineID}`);
      }
      if (p.offset !== -p.length || p.length > this.before.length || this.composition !== "") {
        this.problems.push(
          `deleteSurroundingText(${p.offset}, ${p.length}) with ${this.before.length} units before the caret`,
        );
      }
      this.before = this.before.slice(0, this.before.length - p.length);
      return this.changed();
    },
    setComposition: (p: { contextID: number; text: string; cursor: number }) => {
      this.check(p.contextID, "setComposition");
      if (p.cursor !== p.text.length) {
        this.problems.push(`setComposition cursor ${p.cursor} in ${p.text.length} units`);
      }
      this.composition = p.text;
      return this.changed();
    },
    clearComposition: (p: { contextID: number }) => {
      this.check(p.contextID, "clearComposition");
      this.composition = "";
      return this.changed();
    },
    keyEventHandled: (requestId: string, handled: boolean) => {
      this.answers.set(requestId, handled);
    },
  };

  readonly runtime: { getURL: (path: string) => string };

  constructor(readonly extension: URL, readonly mode: Mode, readonly engine: string) {
    this.runtime = { getURL: (path: string) => new URL(path, extension).href };
  }

  check(contextID: number, call: string): void {
    if (contextID !== this.contextID) {
      this.problems.push(`${call} for context ${contextID} in context ${this.contextID}`);
    }
  }

  changed(): Promise<boolean> {
    const shown = this.before + (this.mode.includeComposition ? this.composition : "");
    const start = Math.max(0, shown.length - READING_LIMIT);
    const caret = shown.length - start;
    this.readings.push({ text: shown.slice(start), focus: caret, anchor: caret, offset: start });
    return Promise.resolve(true);
  }

  #deliver(readings: SurroundingInfo[]): void {
    for (const info of readings) {
      this.ime.onSurroundingTextChanged.fire(this.engine, info);
    }
  }

  // After a key: deliver what it changed, or with lag what the key before
  // it changed.
  settle(): void {
    if (this.mode.lag) {
      const late = this.late;
      this.late = this.readings;
      this.readings = [];
      this.#deliver(late);
    } else {
      this.flush();
    }
  }

  flush(): void {
    const all = [...this.late, ...this.readings];
    this.late = [];
    this.readings = [];
    this.#deliver(all);
  }

  async answer(requestId: string): Promise<boolean> {
    for (let i = 0; i < 10_000; i++) {
      const handled = this.answers.get(requestId);
      if (handled !== undefined) {
        this.answers.delete(requestId);
        return handled;
      }
      await new Promise((resolve) => setTimeout(resolve, 0));
    }
    throw new Error(`key event ${requestId} was never answered`);
  }

  focus(): void {
    this.ime.onBlur.fire(this.contextID);
    this.contextID += 1;
    this.before = "";
    this.composition = "";
    this.readings = [];
    this.late = [];
    this.ime.onFocus.fire({ contextID: this.contextID, type: "text" });
    this.changed();
    this.settle();
  }
}

async function run(fake: FakeChrome, c: Case): Promise<string | null> {
  fake.focus();
  const handled: boolean[] = [];
  for (const event of c.events) {
    fake.requests += 1;
    const id = String(fake.requests);
    const result = fake.ime.onKeyEvent.fire(fake.engine, event, id)[0];
    const answer = result ?? await fake.answer(id);
    if (event.type === "keydown") {
      handled.push(answer);
    }
    fake.settle();
  }
  fake.flush();
  const actual = { handled, text: fake.before, preedit: fake.composition };
  const expected = { handled: c.handled, text: c.text, preedit: c.preedit };
  if (JSON.stringify(actual) === JSON.stringify(expected)) {
    return null;
  }
  return `${c.label}: engine ${JSON.stringify(expected)}, extension ${JSON.stringify(actual)}`;
}

const casesPath = Deno.env.get("KBDGEN_CHROMEOS_CASES");

Deno.test({
  name: "the generated extension types what kbd-engine types",
  ignore: casesPath === undefined,
  async fn() {
    const cases: { fixtures: Fixture[] } = JSON.parse(await Deno.readTextFile(casesPath!));
    const failures: string[] = [];
    let runs = 0;
    for (const [m, mode] of MODES.entries()) {
      for (const [f, fixture] of cases.fixtures.entries()) {
        const extension = pathToFileURL(fixture.extension + "/");
        const fake = new FakeChrome(extension, mode, fixture.engine);
        Object.assign(globalThis, { chrome: { input: { ime: fake.ime }, runtime: fake.runtime } });
        // A query makes each import a fresh instance of the same file.
        await import(new URL(`background.js?mode=${m}&fixture=${f}`, extension).href);
        fake.ime.onActivate.fire(fixture.engine, "normal");
        let failed = 0;
        for (const c of fixture.cases) {
          runs += 1;
          const failure = await run(fake, c);
          if (failure !== null) {
            failed += 1;
            failures.push(`${mode.name}, ${fixture.label}: ${failure}`);
          }
        }
        for (const problem of new Set(fake.problems)) {
          failures.push(`${mode.name}, ${fixture.label}: ${problem}`);
        }
        console.log(
          `${mode.name}, ${fixture.label}: ${fixture.cases.length - failed} of ${fixture.cases.length} cases agree`,
        );
      }
    }
    console.log(`${runs} case runs, ${failures.length} failures`);
    if (failures.length > 0) {
      throw new Error(
        `${failures.length} failures:\n${failures.slice(0, 40).join("\n")}`,
      );
    }
  },
});
