# Unload the speech model after a stretch of no use

`tuh-01M4H2EGFCA1P8SKK5HCASMC0T`

- **Status:** open — ready
- **Priority:** 3
- **Labels:** `engine` `perf`
- **Created:** 2026-10-09 19:32 UTC by `brandon`

## Description

Context: DESIGN.md "Keeping it light and fast" and "Next steps" 8: keep the model loaded so recording starts instantly, and unload it after a stretch of no use. Loaded, the app uses ~690 MB; the shell alone idles around 22 MB. A reload takes ~0.2 s once the shaders are compiled.

The ask: unload after an idle timeout (a setting, with a sensible default), and reload on the next key press while capturing audio from key-down, so no words are lost during the reload.

Acceptance: after the timeout, resident memory falls close to the no-model baseline; a dictation started right after an unload keeps its first words (test with WavReplay); the timeout is configurable.

Pointers: crates/easyvox-app/src/controller.rs; crates/easyvox-core/src/session.rs, tcpp.rs; DESIGN.md "Engine and macOS gotchas" (quitting with a model loaded; ending a session blocks, so drop sessions off the UI thread).

## History

_No activity yet._
