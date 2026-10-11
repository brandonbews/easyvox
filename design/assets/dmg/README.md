# EasyVox — DMG background

| File | Size | Use |
|---|---|---|
| `background.png` | 660 × 400 px | 1× picture |
| `background@2x.png` | 1320 × 800 px | 2× picture (Retina) |

Combine both into one HiDPI TIFF; Finder picks the right resolution for the screen:

```sh
tiffutil -cathidpicheck background.png background@2x.png -out background.tiff
```

The PNGs hold only the paper, the arrow (a grey line that runs through the vermilion wave) and the line
“Drag EasyVox to Applications”. Finder draws the app icon, the Applications alias and the file names on top.

## Finder window settings

| Setting | Value |
|---|---|
| Window content size | 660 × 400 pt — the same as the picture |
| View | Icon view. Toolbar, sidebar, path bar and status bar hidden |
| Icon size | 128 |
| Text size | 13 |
| Label position | Bottom |
| Arrange by | None |
| `EasyVox.app` position (icon centre) | 170, 164 |
| `Applications` alias position (icon centre) | 490, 164 |
| Background | `background.tiff`, stored in a hidden `.background` folder on the volume |
| Volume name | EasyVox |
| Disk image | `EasyVox-1.0.dmg` |

Example with [create-dmg](https://github.com/create-dmg/create-dmg):

```sh
create-dmg \
  --volname "EasyVox" \
  --background "background.tiff" \
  --window-pos 200 120 \
  --window-size 660 400 \
  --icon-size 128 \
  --text-size 13 \
  --icon "EasyVox.app" 170 164 \
  --hide-extension "EasyVox.app" \
  --app-drop-link 490 164 \
  "EasyVox-1.0.dmg" "dist/"
```

## Notes

- **Only a light picture ships.** A DMG background can’t switch with the system appearance, and with a background
  picture set, Finder draws the file names in black and keeps the icon area light, even in Dark Mode. On a dark
  picture the labels would drop to about 1.2:1. The light picture keeps them at about 19:1 and looks right on both desktops.
- Contrast on the palest part of the paper (#F5F5F7): instruction text #6E6E73 4.7:1, arrow line #858589
  3.4:1, wave #F0462D 3.4:1.
- Some tools size the window *including* the title bar. If the bottom of the picture is cut off, add the title-bar
  height (about 28 pt) to the window height. Check the finished image on macOS 26 before release, including Dark Mode.
- The instruction text was rendered in Inter. Re-export from the design source if you want a different face.
