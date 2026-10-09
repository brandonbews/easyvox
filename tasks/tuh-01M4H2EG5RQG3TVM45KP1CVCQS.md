# Decide: make the GitHub repo public, or set an Actions spending limit

`tuh-01M4H2EG5RQG3TVM45KP1CVCQS`

- **Status:** on hold — deliberately paused
- **Priority:** 1
- **Labels:** `human` `decision` `ci`
- **Created:** 2026-10-09 19:32 UTC by `brandon`

## Description

Brandon's call; agents release it. Record the decision in this description and mark it done.

Context: brandonbews/easyvox is private. CI's main job runs on macOS runners, and private-repo macOS minutes bill at 10× the Linux rate (ci.yml runs rustfmt on Linux first for this reason). Public repos run Actions for free. The README already presents EasyVox as free and open source (MIT).

Options:
- Make it public now.
- Stay private and set an Actions spending limit in the account's billing settings.
- Stay private and trim CI (PRs and manual runs only).

Pushing the local commits, and Dependabot PRs if adopted, start macOS CI runs.

## History

_No activity yet._
