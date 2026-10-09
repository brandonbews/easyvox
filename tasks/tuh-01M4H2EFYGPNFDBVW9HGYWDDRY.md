# Hands-on: ⌥Space into TextEdit — focus stays, preview streams, paste lands

`tuh-01M4H2EFYGPNFDBVW9HGYWDDRY`

- **Status:** on hold — deliberately paused
- **Priority:** 0
- **Labels:** `human` `hands-on`
- **Depends on:** [`tuh-py6m`](tuh-01M4H2EFXEY2V2A097BVAVPY6M.md) (on hold)
- **Created:** 2026-10-09 19:32 UTC by `brandon`

## Description

Brandon's task; agents release it. Mark it done when finished, and file a task for anything that fails. Needs the Microphone check first.

Setup: `scripts/bundle.sh && open dist/EasyVox.app` at an unlocked screen. The ad-hoc signature changes on every rebuild, so grant Accessibility (and Microphone) again after each one.

With a TextEdit document focused:
- Hold ⌥Space: the overlay opens at once and the live preview streams while you speak.
- TextEdit keeps focus throughout: its caret stays active, the overlay never takes the keyboard.
- Release: the final text pastes at the cursor and stays on the clipboard.
- The dictation appears in History.
- Without Accessibility granted, EasyVox shows an error instead of silently dropping the paste.

Pointers: docs/ui-stack-spike.md "Hands-on checks (pending)"; DESIGN.md "Paste at cursor".

## History

_No activity yet._
