# Changelog

Notable changes in each release, newest first, in the [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) format. Versions match the `version` in `Cargo.toml`.

## Unreleased

The first release, for Apple silicon Macs on macOS 26 or later.

### Added

- Push-to-talk dictation: hold ⌥Space (or right ⌥, an option in Settings) and speak. An overlay above the Dock shows the words as they're recognized, and letting go pastes the text at the cursor.
- Speech recognition runs on the Mac, with Parakeet TDT 0.6B v2 (the default) or Whisper base.en. No audio or text leaves the Mac.
- Models download in the app and are checked against their SHA-256. An idle model is freed after 10 minutes, unless Keep model loaded is on.
- First-launch setup asks for the Microphone and Accessibility, then downloads the default model and loads it once.
- Settings, and a History of past dictations with a copy button for each.
- A settings file, `~/.config/easyvox/config.toml`, applied as soon as it's saved.
- Check for Updates…, in the menu bar menu and in Settings › About. It reads `easyvox.dev/latest.json`, which each release commits to the site, and when a newer version is out, offers to open its download page. It runs only when you click it and installs nothing.
