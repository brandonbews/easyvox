# Hands-on: overlay level meter, 'Can't hear you', live light/dark

`tuh-01M4KNXEYGGE3QSN96GHNXQJQS`

- **Status:** on hold — deliberately paused
- **Priority:** 2
- **Labels:** `human` `hands-on`
- **Created:** 2026-10-10 19:51 UTC by `brandon`

## Description

Brandon's check at an unlocked screen; agents never take it. Built in b9dd9fb. Setup: `scripts/bundle.sh && open dist/EasyVox.app` (grant Accessibility and Microphone again after a rebuild).

1. Dictate: does the meter at 15 updates a second look alive? If not, try meter_interval_ms = 33 under [advanced] in ~/.config/easyvox/config.toml (applies live).
2. Mute the mic (or pick a silent input) and press: 'Can't hear you' by about 1.5 s. With AirPods it shouldn't flash at the start.
3. Switch Appearance (System Settings > Appearance) mid-dictation: the pill recolours live.
4. Light mode: is the grey provisional text readable over bright and over dark windows?
5. Optional, the unlocked cost: run macmon while `open -W dist/EasyVox.app --args --self-test --replay <absolute path to a clip>` (self-test build), meter on vs meter_interval_ms = 0.

## History

_No activity yet._
