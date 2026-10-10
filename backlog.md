# Backlog

0 in progress · 14 ready · 6 blocked · 13 on hold · 9 inbox · 13 done · 0 cancelled

## In progress

_None._

## Ready

| ID | Task | Priority | Labels |
|---|---|---:|---|
| [`tuh-1wfq`](tasks/tuh-01M4H2EG840VHRYSSV8KJE1WFQ.md) | Skip the final pass when a recording has no speech | 1 | `engine` |
| [`tuh-x7tv`](tasks/tuh-01M4H2EG8YM9TXGK81DC6WX7TV.md) | Core: download a model in the app — download, sha256 verify, activate | 1 | `models` `engine` |
| [`tuh-2bj9`](tasks/tuh-01M4HT8MER13BDAFEFBZG62BJ9.md) | Widen the chunk-join slack so soft words at a cut aren't lost | 1 | `engine` |
| [`tuh-z7dr`](tasks/tuh-01M4HVK6QMYNHCMAGZ57G0Z7DR.md) | Make dictation light: tune the preview until the GPU mostly rests while talking | 1 | `engine` `perf` `energy` |
| [`tuh-aeba`](tasks/tuh-01M4H2EG7A8NGQH489P244AEBA.md) | Core: TranscriptEvent::Level, and a level meter in the overlay | 2 | `engine` `overlay` |
| [`tuh-y7kh`](tasks/tuh-01M4H2EGBBZHN7RRYYS193Y7KH.md) | Overlay: follow the system light/dark appearance | 2 | `overlay` |
| [`tuh-k2w3`](tasks/tuh-01M4H2EGCYFBHHG4S9307GK2W3.md) | Show each model's license and attribution in the app (NVIDIA notice for Parakeet Unified) | 2 | `models` `app` |
| [`tuh-vred`](tasks/tuh-01M4H2EGEJ4R8B326AEDY4VRED.md) | Overlay animation: open and close, words easing in, Reduce motion | 3 | `overlay` |
| [`tuh-ckcx`](tasks/tuh-01M4H2EGH07D79BJEV7Y3NCKCX.md) | Quiet the gpui "RefCell already borrowed" error logged at every launch | 3 | `app` |
| [`tuh-04zz`](tasks/tuh-01M4H2EGHS8QGWQCZ7TGRT04ZZ.md) | Check, and likely set, MACOSX_DEPLOYMENT_TARGET=26.0 in .cargo/config.toml | 3 | `build` |
| [`tuh-y75b`](tasks/tuh-01M4H2EGJKYV58RHW70R9YY75B.md) | Future-incompat warning from the block 0.1.6 dependency | 3 | `deps` |
| [`tuh-66zt`](tasks/tuh-01M4H2EGKDEZ7NPDTDRYRD66ZT.md) | Dependabot for the GitHub Actions SHA pins | 3 | `ci` `deps` |
| [`tuh-8kk7`](tasks/tuh-01M4HDR425AX71RAQFA15Q8KK7.md) | Whisper base.en preview: heavy flicker and growing CPU | 3 | `engine` `models` |
| [`tuh-0jj5`](tasks/tuh-01M4HDR42WSW44E2TM6C940JJ5.md) | Drop or use DecodeMode | 4 | `engine` `cleanup` |

## Blocked / waiting

| ID | Task | Priority | Waiting on |
|---|---|---:|---|
| [`tuh-br58`](tasks/tuh-01M4H2EG9QB2ETECD30F9RBR58.md) | Onboarding: Microphone and Accessibility, default model download, shader warm-up | 1 | depends on [`tuh-x7tv`](tasks/tuh-01M4H2EG8YM9TXGK81DC6WX7TV.md) |
| [`tuh-gz9m`](tasks/tuh-01M4H2EGAHMQ8590ASYQ6FGZ9M.md) | Settings: click-to-download the alternative models | 2 | depends on [`tuh-x7tv`](tasks/tuh-01M4H2EG8YM9TXGK81DC6WX7TV.md) |
| [`tuh-stg4`](tasks/tuh-01M4H2EGC4XDQT10EWCXX1STG4.md) | Settings file: ~/.config/easyvox/config.toml, applied live | 1 | depends on [`tuh-z7dr`](tasks/tuh-01M4HVK6QMYNHCMAGZ57G0Z7DR.md) |
| [`tuh-btdp`](tasks/tuh-01M4H2EGNVZ00YA6604MXABTDP.md) | Shipping build: cargo-packager for Developer ID signing, notarization and the DMG | 3 | depends on [`tuh-2645`](tasks/tuh-01M4H2EGM7TG4S96QNRQHV2645.md) |
| [`tuh-dtts`](tasks/tuh-01M4H2EGPP2HDY5Y1WD6ENDTTS.md) | Homebrew cask for the notarized DMG | 4 | depends on [`tuh-btdp`](tasks/tuh-01M4H2EGNVZ00YA6604MXABTDP.md) |
| [`tuh-b8rb`](tasks/tuh-01M4HDR400G242K017BJ1HB8RB.md) | Energy spike: GPU vs cheaper preview vs Neural Engine | 2 | depends on [`tuh-z7dr`](tasks/tuh-01M4HVK6QMYNHCMAGZ57G0Z7DR.md) |

## On hold

Triaged, deliberately paused — never served to agents until reopened.

| ID | Task | Priority | Labels |
|---|---|---:|---|
| [`tuh-py6m`](tasks/tuh-01M4H2EFXEY2V2A097BVAVPY6M.md) | Hands-on: Microphone permission prompt, then real mic dictation | 0 | `human` `hands-on` |
| [`tuh-ddry`](tasks/tuh-01M4H2EFYGPNFDBVW9HGYWDDRY.md) | Hands-on: ⌥Space into TextEdit — focus stays, preview streams, paste lands | 0 | `human` `hands-on` |
| [`tuh-v4xx`](tasks/tuh-01M4H2EFZA72VNDBQ1G6FHV4XX.md) | Hands-on: Settings and History come to the front; model dropdown and shortcut recorder | 0 | `human` `hands-on` |
| [`tuh-zft3`](tasks/tuh-01M4H2EG0XAV06KW9567DVZFT3.md) | Hands-on: live light/dark switch | 1 | `human` `hands-on` |
| [`tuh-pxhr`](tasks/tuh-01M4H2EG1Q0RW4QKRD5YH9PXHR.md) | Hands-on: hold right ⌥ as the shortcut (Input Monitoring, relaunch) | 1 | `human` `hands-on` |
| [`tuh-dw5x`](tasks/tuh-01M4H2EG2HY0EK9T9EFHSEDW5X.md) | Hands-on: overlay and paste over full-screen apps and in other Spaces | 1 | `human` `hands-on` |
| [`tuh-h9ja`](tasks/tuh-01M4H2EG3AT1WSW8YRH9DXH9JA.md) | Hands-on: the debug "Test dictation" menu item | 1 | `human` `hands-on` |
| [`tuh-2645`](tasks/tuh-01M4H2EGM7TG4S96QNRQHV2645.md) | Create a Developer ID Application certificate and a notarization key | 3 | `human` `release` |
| [`tuh-tz3a`](tasks/tuh-01M4H2EGN1Y7XVCGM2JVNSTZ3A.md) | Decide: which auto-updater | 3 | `human` `decision` `release` |
| [`tuh-xm3z`](tasks/tuh-01M4H2EGQHWYDW7WSWNPARXM3Z.md) | Upgrade transcribe-cpp to 0.4 for Parakeet Ultra (French) | 4 | `models` `engine` |
| [`tuh-n7dz`](tasks/tuh-01M4H2EGRBQ8Q57SAH4YT0N7DZ.md) | Decide: how LLM cleanup gets around the one-ggml-per-binary rule | 4 | `human` `decision` `engine` |
| [`tuh-h2jj`](tasks/tuh-01M4HDR43KDP1NSH6C16DFH2JJ.md) | Offer the ggml-metal idle-loop patch upstream | 4 | `human` `engine` `upstream` |
| [`tuh-x9cd`](tasks/tuh-01M4HN1AWAEEECAS8SWAJ5X9CD.md) | Hands-on: 'Keep model loaded' switch, and first words after an unload | 1 | `human` `hands-on` |

## Inbox

Untriaged captures — promoting one to open means writing it a real (prompt-quality) description first.

- [`tuh-2pbx`](tasks/tuh-01M4H2EGS5QBS67W1BJJPQ2PBX.md) Toggle mode: tap to start, tap to stop, for long dictation
- [`tuh-hfmv`](tasks/tuh-01M4H2EGSZKGDNPJ6ZGGWYHFMV.md) History: search, delete and a retention limit
- [`tuh-ptyf`](tasks/tuh-01M4H2EGTSD1KPFMY613SMPTYF.md) Setting: restore the previous clipboard after the paste
- [`tuh-qkpv`](tasks/tuh-01M4H2EGVNSR86AXDM3P8RQKPV.md) Paste on non-QWERTY layouts: ⌘V is sent as key code 9
- [`tuh-mpwk`](tasks/tuh-01M4H2EGWGMR0HM5FB804AMPWK.md) Paste fallbacks for apps where ⌘V misbehaves
- [`tuh-y9nq`](tasks/tuh-01M4H2EGXBSWC0ZN3A6DDXY9NQ.md) Set a latency target once real-mic numbers exist
- [`tuh-rvra`](tasks/tuh-01M4H2EGY7M3P5SK0T6XVARVRA.md) Speak French, get English
- [`tuh-bdwr`](tasks/tuh-01M4H2EGZ33FCMX66PPPCBBDWR.md) Linux port (X11 first; Wayland needs per-desktop work)
- [`tuh-k3k5`](tasks/tuh-01M4HT8MFHW8DYWZ16Z7JYK3K5.md) Pause finder: add a noise-floor term to quiet_level (floor + 6 dB) if louder rooms start forcing cuts

## Done

- [`tuh-1xne`](tasks/tuh-01M4H2EG038JCXHPQYECTZ1XNE.md) Multi-minute dictations: preview and final text hold up when the key is held for minutes
- [`tuh-838p`](tasks/tuh-01M4H2EG449Q4HK90B08MQ838P.md) Hands-on: compare Parakeet Unified, TDT v2 and Unified at 160 ms chunks, and pick the default
- [`tuh-h9ae`](tasks/tuh-01M4H2EG4X1ZDTHE9QVGJ5H9AE.md) Hands-on: record a few real-voice clips for benching
- [`tuh-vcqs`](tasks/tuh-01M4H2EG5RQG3TVM45KP1CVCQS.md) Decide: make the GitHub repo public, or set an Actions spending limit
- [`tuh-z2g1`](tasks/tuh-01M4H2EG6HXSP5GCH39FEPZ2G1.md) Push the 7 local commits on main (and let tuhdoo publish its data branch)
- [`tuh-bdnm`](tasks/tuh-01M4H2EGDR663G3XGTBA14BDNM.md) Bench the model list on the real-voice clips
- [`tuh-mc0t`](tasks/tuh-01M4H2EGFCA1P8SKK5HCASMC0T.md) Unload the speech model after a stretch of no use
- [`tuh-ksmt`](tasks/tuh-01M4H2EGG52WC4E29B82S4KSMT.md) Idle wake-ups from ggml-metal's residency loop
- [`tuh-vbtm`](tasks/tuh-01M4HDR3YH6F52Q77NJFQ7VBTM.md) Decide the default model: Parakeet TDT v2 or Unified
- [`tuh-4p1s`](tasks/tuh-01M4HDR3Z8GQ7132ZNSN5Q4P1S.md) Skip preview passes when nothing new was heard
- [`tuh-6htq`](tasks/tuh-01M4HDR40PA99G6HQDAN026HTQ.md) Test chunk cutting on real-voice long recordings; consider a VAD
- [`tuh-6x7y`](tasks/tuh-01M4HDR41EE6Y4XXYSW3436X7Y.md) Add the long-dictation clip generator to testdata/
- [`tuh-wa8j`](tasks/tuh-01M4HET091632AY8RKAC4NWA8J.md) Make Parakeet TDT v2 the default model

## Cancelled

_None._
