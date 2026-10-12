# Settings

EasyVox reads one TOML file: `~/.config/easyvox/config.toml`, or `$XDG_CONFIG_HOME/easyvox/config.toml` when that's set. The Settings window writes the same file. Changes apply when you save, with no restart.

Set only the keys you want to change. Keys you leave out use their defaults, so a release that improves a default reaches you.

## In the Settings window

| Key | Default | Effect |
|---|---|---|
| `shortcut` | `"alt-space"` | Hold to dictate, release to paste. Modifiers (`ctrl`, `alt`, `shift`, `cmd`) and a key, joined by `-`: `"ctrl-shift-space"`, `"f5"`. |
| `hold_right_option` | `false` | Also dictate while right ⌥ is held on its own. Needs Input Monitoring. |
| `model` | `"parakeet-tdt-0.6b-v2-q5_k_m-gguf"` | The speech model. The other one offered is `"whisper-base.en-q8_0-gguf"`. |
| `keep_model_loaded` | `false` | Keep the model in memory. When off, the model is freed after `unload_after_secs` and reloads (about 0.15 s) when you next press the shortcut. Recording starts at once either way. |

## Under `[advanced]`

| Key | Default | Effect |
|---|---|---|
| `unload_after_secs` | `600` | Idle seconds before the model is freed: with the default model, about 670 MB down to 90 MB. 1 to 86400. |
| `overlay_hide_after_final_ms` | `600` | How long the pill stays up after pasting. 0 to 10000. |
| `overlay_dock_gap_pt` | `24` | Gap between the Dock and the pill, in points. 0 to 400. |
| `preview_interval_ms` | `1600` | Time between preview updates. Lower updates more often and uses more power. |
| `overlay_open_ms`, `overlay_close_ms`, `overlay_words_ms`, `overlay_glide_ms`, `overlay_meter_ms` | `200`, `120`, `200`, `250`, `120` | The pill's animations. `0` turns one off. With macOS's Reduce motion on, the pill only fades. |

## Example

```toml
shortcut = "ctrl-shift-space"
keep_model_loaded = true

[advanced]
overlay_hide_after_final_ms = 1500
overlay_open_ms = 0
overlay_close_ms = 0
```

## Errors

EasyVox never rewrites your file to fix it.

- An invalid value falls back to that key's default.
- An unknown key is ignored, with a suggestion when it looks like a typo.
- A file that isn't valid TOML is ignored until you fix it, and the previous settings stay.

Problems show under **Settings › General › Settings file**.

## Every key

[Settings reference](settings-reference.md) lists all of them, with defaults, ranges and trade-offs. The app prints the same list:

```sh
/Applications/EasyVox.app/Contents/MacOS/easyvox --print-default-config
```
