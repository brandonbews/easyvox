// The bench numbers behind /results, read at build time from the CSVs in ../docs/bench (written
// by scripts/bench/extract.py; docs/bench/README.md says what each column holds). The browser
// never fetches them. Which runs each part of the page uses is pinned in results.config.ts.
//
// Three guards, each failing the build loudly:
//   - a schema per file: every column the page reads must be there, so a column renamed by the
//     extractor stops the build instead of emptying a chart;
//   - every cell must be empty, a number or a short label (no spaces, at most 80 characters).
//     The CSVs hold numbers and fixed labels only; a cell that looks like free text, such as a
//     line of a transcript, is refused, and the error names its row and column, never its text;
//   - every pinned selection must find exactly the runs it expects.

import fs from "node:fs";
import path from "node:path";
import { REPO_ROOT } from "./docs";
import { accuracy, cost, idle, MODELS, type ModelPin, silence, speed } from "./results.config";

const BENCH_DIR = path.join(REPO_ROOT, "docs", "bench");

type Kind = "number" | "label";
type Row = Record<string, string>;

// The columns the page reads, per file. Other columns may come and go, but still pass the cell check.
const SCHEMAS: Record<string, Record<string, Kind>> = {
  "runs.csv": {
    run_id: "label",
    study: "label",
    build: "label",
    model: "label",
    clip: "label",
    wer_errors: "number",
    wer_pct: "number",
    ref_words: "number",
    speech_onset_s: "number",
    first_words_ms: "number",
    finalize_ms: "number",
  },
  "energy.csv": { run_id: "label", j_per_min: "number" },
  "memory.csv": {
    run_id: "label",
    phase: "label",
    footprint_mb: "number",
    peak_footprint_mb: "number",
  },
  "idle.csv": {
    study: "label",
    build: "label",
    state: "label",
    wakeups_per_s: "number",
    cpu_pct: "number",
    footprint_mb: "number",
  },
  "stress.csv": {
    run_id: "label",
    case: "label",
    model: "label",
    speech_found: "number",
    text_kept: "number",
  },
};

const NUMBER = /^-?(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?$/;
const LABEL = /^[A-Za-z0-9][A-Za-z0-9_.+\-/=;]{0,79}$/;

// RFC 4180: comma-separated, fields optionally in double quotes, "" for a quote inside quotes,
// CRLF or LF line ends, an optional final line end.
export function parseCsv(text: string, name = "csv"): string[][] {
  const rows: string[][] = [];
  let row: string[] = [];
  let field = "";
  let quoted = false;
  let i = 0;
  while (i < text.length) {
    const c = text[i];
    if (quoted) {
      if (c === '"' && text[i + 1] === '"') {
        field += '"';
        i += 2;
        continue;
      }
      if (c === '"') quoted = false;
      else field += c;
      i += 1;
      continue;
    }
    if (c === '"' && field === "") quoted = true;
    else if (c === ",") {
      row.push(field);
      field = "";
    } else if (c === "\n" || c === "\r") {
      row.push(field);
      rows.push(row);
      row = [];
      field = "";
      if (c === "\r" && text[i + 1] === "\n") i += 1;
    } else if (c === '"')
      throw new Error(`${name}: a quote inside an unquoted field (row ${rows.length + 1})`);
    else field += c;
    i += 1;
  }
  if (quoted) throw new Error(`${name}: a quoted field never ends`);
  if (field !== "" || row.length > 0) {
    row.push(field);
    rows.push(row);
  }
  return rows;
}

const tables = new Map<string, Row[]>();

// One CSV from docs/bench, checked against its schema and the cell rule, read once per build.
function table(file: string): Row[] {
  const cached = tables.get(file);
  if (cached) return cached;
  const where = `docs/bench/${file}`;
  const schema = SCHEMAS[file];
  if (!schema) throw new Error(`${where}: no schema in src/lib/bench.ts`);
  const [header, ...body] = parseCsv(fs.readFileSync(path.join(BENCH_DIR, file), "utf8"), where);
  if (!header) throw new Error(`${where}: empty file`);
  for (const column of Object.keys(schema)) {
    if (!header.includes(column)) {
      throw new Error(
        `${where}: no column "${column}" (renamed by the extractor?); /results reads it`,
      );
    }
  }
  const rows = body.map((cells, r) => {
    const line = r + 2;
    if (cells.length !== header.length) {
      throw new Error(
        `${where}: line ${line} has ${cells.length} cells, the header ${header.length}`,
      );
    }
    const row: Row = {};
    header.forEach((column, c) => {
      const cell = cells[c];
      if (cell !== "") {
        const kind = schema[column];
        const ok = kind === "number" ? NUMBER.test(cell) : NUMBER.test(cell) || LABEL.test(cell);
        if (!ok) {
          // Never echo the cell: if it's text that got in by mistake, it stays out of the log.
          throw new Error(
            `${where}: line ${line}, column "${column}" holds a ${cell.length}-character value that ` +
              `isn't ${kind === "number" ? "a number" : "a number or a short label"}. The bench CSVs ` +
              "hold numbers and fixed labels only, never text.",
          );
        }
      }
      row[column] = cell;
    });
    return row;
  });
  tables.set(file, rows);
  return rows;
}

function num(row: Row, column: string): number {
  const cell = row[column];
  if (cell === undefined || cell === "") {
    throw new Error(`run ${row.run_id ?? "?"}: no ${column}, which /results needs`);
  }
  return Number(cell);
}

function expectCount<T>(items: T[], count: number, what: string): T[] {
  if (items.length !== count) {
    throw new Error(
      `/results expects ${count} ${what}, and docs/bench has ${items.length}. ` +
        "Update src/lib/results.config.ts on purpose to show other runs.",
    );
  }
  return items;
}

function runsOf(
  pin: { study: string; build: string },
  model: ModelPin,
  clip: string,
  count: number,
): Row[] {
  const rows = table("runs.csv").filter(
    (r) =>
      r.study === pin.study && r.build === pin.build && r.model === model.id && r.clip === clip,
  );
  return expectCount(rows, count, `${pin.study} runs of ${model.id} on ${clip}`);
}

// ---------- formatting, rounded as docs/model-bench.md rounds ----------

// A number with `decimals` places, rounding half to even on the double's exact value, as Python's
// formatting does (the doc's tables were made with Python). toFixed rounds halves up: 1988.5 MB
// would print as 1,989 where the doc says 1,988.
export function fixed(x: number, decimals: number): string {
  const exact = Math.abs(x).toFixed(Math.min(100, decimals + 40));
  const [whole, frac] = exact.split(".");
  const digits = (whole + frac.slice(0, decimals)).split("").map(Number);
  const rest = frac.slice(decimals);
  const tie = /^50*$/.test(rest);
  const up =
    rest[0] > "5" || (rest[0] === "5" && !tie) || (tie && digits[digits.length - 1] % 2 === 1);
  if (up) {
    let i = digits.length - 1;
    while (i >= 0 && digits[i] === 9) digits[i--] = 0;
    if (i < 0) digits.unshift(1);
    else digits[i] += 1;
  }
  const s = digits.join("");
  const cut = s.length - decimals;
  const out = decimals > 0 ? `${s.slice(0, cut)}.${s.slice(cut)}` : s;
  return x < 0 && /[1-9]/.test(out) ? `-${out}` : out;
}

// "3557" → "3,557"
function thousands(s: string): string {
  const [whole, frac] = s.split(".");
  const grouped = whole.replace(/\B(?=(\d{3})+$)/g, ",");
  return frac === undefined ? grouped : `${grouped}.${frac}`;
}

// "0.60–0.64", or one value when both ends round the same.
function range(values: number[], decimals: number, group = false): string {
  const fmt = (n: number) => (group ? thousands(fixed(n, decimals)) : fixed(n, decimals));
  const lo = fmt(Math.min(...values));
  const hi = fmt(Math.max(...values));
  return lo === hi ? lo : `${lo}–${hi}`;
}

const mean = (values: number[]) => values.reduce((a, b) => a + b, 0) / values.length;
const median = (values: number[]) => {
  const s = [...values].sort((a, b) => a - b);
  const m = Math.floor(s.length / 2);
  return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2;
};

// ---------- the page's data ----------

// Word errors on the read-aloud passage. Every model wrote the same text in its 3 runs, so the
// count is the same in each; a model whose runs disagree fails the build.
export function accuracyRows() {
  return MODELS.map((model) => {
    const runs = runsOf(accuracy, model, accuracy.clip, accuracy.runsPerModel);
    const errors = new Set(runs.map((r) => num(r, "wer_errors")));
    const wer = new Set(runs.map((r) => num(r, "wer_pct")));
    const ref = new Set(runs.map((r) => num(r, "ref_words")));
    if (errors.size !== 1 || wer.size !== 1 || ref.size !== 1 || !ref.has(accuracy.refWords)) {
      throw new Error(
        `${model.id}: its read-aloud runs disagree, or aren't scored on ${accuracy.refWords} words`,
      );
    }
    const [count] = [...errors];
    const [rate] = [...wer];
    return { ...model, value: count, display: `${count}`, rate: `${fixed(rate, 2)} %` };
  }).sort((a, b) => a.value - b.value || Number.parseFloat(a.rate) - Number.parseFloat(b.rate));
}

// First words after speech starts, and text ready after release, on the read-aloud passage
// (3 runs: the bar is the median, the label the range), plus text ready on the ramble.
export function speedRows() {
  return MODELS.map((model) => {
    const runs = runsOf(speed, model, speed.clip, speed.runsPerModel);
    const first = runs.map(
      (r) => (num(r, "first_words_ms") - num(r, "speech_onset_s") * 1000) / 1000,
    );
    const ready = runs.map((r) => num(r, "finalize_ms") / 1000);
    const ramble = runsOf(speed, model, speed.rambleClip, model.rambleRuns).map(
      (r) => num(r, "finalize_ms") / 1000,
    );
    return {
      ...model,
      first: { value: median(first), display: `${range(first, 2)} s` },
      ready: { value: median(ready), display: `${range(ready, 2)} s` },
      rambleReady: `${range(ramble, 2)} s`,
    };
  }).sort((a, b) => a.ready.value - b.ready.value);
}

// Energy per minute of the ramble (the mean of its runs; the range where there are two), and
// memory on the read-aloud runs: the footprint once loaded, and the peak.
export function costRows() {
  const energy = new Map(table("energy.csv").map((r) => [r.run_id, r]));
  const memory = table("memory.csv");
  return MODELS.map((model) => {
    const rambles = runsOf(cost, model, cost.energyClip, model.rambleRuns);
    const joules = rambles.map((r) => {
      const e = energy.get(r.run_id);
      if (!e) throw new Error(`${r.run_id}: no row in docs/bench/energy.csv`);
      return num(e, "j_per_min");
    });
    const m = mean(joules);
    const reads = runsOf(cost, model, cost.memoryClip, cost.memoryRunsPerModel);
    const phase = (runId: string, name: string) => {
      const rows = memory.filter((r) => r.run_id === runId && r.phase === name);
      return expectCount(rows, 1, `"${name}" memory rows for ${runId}`)[0];
    };
    const loaded = reads.map((r) => num(phase(r.run_id, "loaded"), "footprint_mb"));
    const peak = reads.map((r) => num(phase(r.run_id, "end"), "peak_footprint_mb"));
    return {
      ...model,
      energy: { value: m, display: `${fixed(m, 0)} J` },
      energyRange: joules.length > 1 ? range(joules, m >= 100 ? 0 : 1) : null,
      memory: { value: Math.max(...peak), display: `${range(peak, 0, true)} MB` },
      memoryLoaded: `${range(loaded, 0, true)} MB`,
    };
  }).sort((a, b) => a.energy.value - b.energy.value);
}

// The idle app, with the model loaded and after the unload.
export function idleRows() {
  const pick = (state: string) =>
    expectCount(
      table("idle.csv").filter(
        (r) => r.study === idle.study && r.build === idle.build && r.state === state,
      ),
      1,
      `${idle.study} rows for ${idle.build}, ${state}`,
    )[0];
  return (["loaded", "unloaded"] as const).map((key) => {
    const r = pick(idle.states[key]);
    return {
      key,
      wakeups: num(r, "wakeups_per_s").toString(),
      cpu: `${num(r, "cpu_pct")} %`,
      memory: `${thousands(fixed(num(r, "footprint_mb"), 0))} MB`,
    };
  });
}

// The detector's stress test: per case, the sessions, how many found speech and kept text.
export function silenceRows() {
  const pinned = new Set(
    table("runs.csv")
      .filter((r) => r.study === silence.study && r.build === silence.build)
      .map((r) => r.run_id),
  );
  const rows = table("stress.csv");
  const unpinned = rows.filter((r) => !pinned.has(r.run_id)).length;
  if (unpinned > 0)
    throw new Error(
      `docs/bench/stress.csv: ${unpinned} rows aren't ${silence.study}/${silence.build} runs`,
    );
  expectCount(rows, silence.sessions, "stress sessions");
  return silence.cases.map(({ case: name, sessions, label }) => {
    const of = expectCount(
      rows.filter((r) => r.case === name),
      sessions,
      `"${name}" stress sessions`,
    );
    return {
      label,
      sessions,
      found: of.filter((r) => num(r, "speech_found") === 1).length,
      kept: of.filter((r) => num(r, "text_kept") === 1).length,
      speech: name !== "noise",
    };
  });
}
