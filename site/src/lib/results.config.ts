// What /benchmarks shows, pinned. Each part of the page names the bench study and build its runs
// come from, the recording, the models and how many runs it expects (src/lib/bench.ts reads the
// CSVs in ../docs/bench). A new bench run, a new study or a new build never changes the page by
// itself: either it isn't selected, or the run counts stop matching and the build fails. Showing
// new numbers is a change to this file, made on purpose.
//
// Study and build names are the ones docs/bench/README.md lists ("Studies and builds"). They stay
// here: the page shows no dates, commits or build ids.

export type Tone = "default" | "offered" | "bench";

export type ModelPin = {
  id: string; // the `model` column
  name: string; // as the page names it
  tone: Tone; // vermilion for the default, a dark grey for the other offered model, light grey
  tag?: string; // shown after the name
  rambleRuns: number; // energy runs on the 5-minute recording
};

// The fourteen candidates of "Every candidate, one pipeline". Two of them are offered in the app.
export const MODELS: ModelPin[] = [
  {
    id: "parakeet-tdt-0.6b-v2-q5_k_m-gguf",
    name: "Parakeet TDT v2",
    tone: "default",
    tag: "default",
    rambleRuns: 2,
  },
  {
    id: "whisper-base.en-q8_0-gguf",
    name: "Whisper base.en",
    tone: "offered",
    tag: "small option",
    rambleRuns: 2,
  },
  {
    id: "cohere-transcribe-03-2026-q5_k_m-gguf",
    name: "Cohere Transcribe",
    tone: "bench",
    rambleRuns: 2,
  },
  {
    id: "parakeet-unified-en-0.6b-q5_k_m-gguf",
    name: "Parakeet Unified",
    tone: "bench",
    rambleRuns: 2,
  },
  { id: "parakeet-tdt-0.6b-v3-q5_k_m-gguf", name: "Parakeet TDT v3", tone: "bench", rambleRuns: 2 },
  {
    id: "whisper-large-v3-turbo-q5_k_m-gguf",
    name: "Whisper large-v3-turbo",
    tone: "bench",
    rambleRuns: 2,
  },
  { id: "canary-qwen-2.5b-q5_k_m-gguf", name: "Canary-Qwen 2.5B", tone: "bench", rambleRuns: 1 },
  {
    id: "distil-whisper-large-v3.5-f16-ggml",
    name: "Distil-Whisper large-v3.5",
    tone: "bench",
    rambleRuns: 1,
  },
  {
    id: "granite-speech-4.1-2b-q5_k_m-gguf",
    name: "Granite Speech 4.1 2B",
    tone: "bench",
    rambleRuns: 1,
  },
  {
    id: "moonshine-streaming-medium-q8_0-gguf",
    name: "Moonshine Streaming Medium",
    tone: "bench",
    rambleRuns: 1,
  },
  { id: "qwen3-asr-1.7b-q5_k_m-gguf", name: "Qwen3-ASR 1.7B", tone: "bench", rambleRuns: 1 },
  {
    id: "granite-speech-5.0-470m-turboctc-q5_k_m-gguf",
    name: "Granite Speech 5.0 TurboCTC",
    tone: "bench",
    rambleRuns: 1,
  },
  {
    id: "nemotron-speech-streaming-en-0.6b-q5_k_m-gguf",
    name: "Nemotron Speech Streaming",
    tone: "bench",
    rambleRuns: 1,
  },
  { id: "canary-180m-flash-q5_k_m-gguf", name: "Canary 180M Flash", tone: "bench", rambleRuns: 2 },
];

// "Every candidate, one pipeline": every model through one build, on the same audio.
const CANDIDATES = { study: "candidates", build: "93fc164+catalog" } as const;

// The read-aloud passage (81 s, 238 words with a reference) and the 5-minute ramble.
const READ_ALOUD = "structured.wav";
const RAMBLE = "ramble.wav";

// Word errors on the read-aloud passage: runs.csv `wer_errors`, `wer_pct`, `ref_words`.
export const accuracy = { ...CANDIDATES, clip: READ_ALOUD, runsPerModel: 3, refWords: 238 };

// First words (runs.csv `first_words_ms` less `speech_onset_s`) and text ready after release
// (`finalize_ms`) on the read-aloud passage; text ready on the ramble for the table.
export const speed = {
  ...CANDIDATES,
  clip: READ_ALOUD,
  runsPerModel: 3,
  rambleClip: RAMBLE,
  axisMaxS: 3,
};

// Energy per minute of the ramble (energy.csv `j_per_min`, joined by run_id), and memory on the
// read-aloud runs (memory.csv: `loaded` footprint, `end` peak footprint).
export const cost = {
  ...CANDIDATES,
  energyClip: RAMBLE,
  memoryClip: READ_ALOUD,
  memoryRunsPerModel: 3,
};

// The idle app ("Idle and energy: The idle app"): the build with the three idle changes, with the
// model loaded and after the 10-minute unload. Measured with Parakeet Unified loaded.
export const idle = {
  study: "idle-app",
  build: "d33ab36",
  states: { loaded: "loaded", unloaded: "unloaded" },
};

// The speech detector's stress test ("Quiet mics and loud rooms"), on the final detector. stress.csv
// has no study or build column, so its rows are matched to runs.csv by run_id.
export const silence = {
  study: "stress",
  build: "new3",
  sessions: 243,
  // The page's rows, in order, with how many sessions each should have.
  cases: [
    {
      case: "quieter",
      sessions: 73,
      label: "Speech 10–30 dB quieter, as from a mic turned well down",
    },
    { case: "edge", sessions: 40, label: "Speech only 10 dB louder than the room" },
    { case: "noisy", sessions: 46, label: "Speech under loud noise, or other people talking" },
    { case: "soft", sessions: 72, label: "One soft word on its own" },
    { case: "noise", sessions: 12, label: "Silence, or room noise with nobody speaking" },
  ],
};
