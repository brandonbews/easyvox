# Decide: how LLM cleanup gets around the one-ggml-per-binary rule

`tuh-01M4H2EGRBQ8Q57SAH4YT0N7DZ`

- **Status:** on hold — deliberately paused
- **Priority:** 4
- **Labels:** `human` `decision` `engine`
- **Created:** 2026-10-09 19:32 UTC by `brandon`

## Description

Brandon's call; agents release it. Record the decision in this description and mark it done (DESIGN.md "Open questions").

Context: DESIGN.md "The two model approach" (A) and "Engine": optional LLM cleanup (filler words, punctuation, self-corrections, lists), off by default and loaded only when on, with the raw text kept in History. llama.cpp bindings vendor their own ggml, and only one ggml can be linked into a binary next to transcribe.cpp; the linker quietly keeps one copy, so it fails at runtime, not at build time.

Options:
- A separate helper process with llama.cpp, launched only while cleanup is on.
- A llama.cpp build that shares transcribe.cpp's ggml (versions in lockstep).
- Apple's Foundation Models framework through a Swift bridge: macOS 26 on Apple Intelligence Macs, no disk or memory cost to us, Mac only.

## History

_No activity yet._
