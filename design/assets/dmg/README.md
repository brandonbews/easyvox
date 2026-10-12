# EasyVox DMG background

| File | Size | Use |
|---|---|---|
| `background.png` | 660 × 400 px | 1× picture |
| `background@2x.png` | 1320 × 800 px | 2× picture (Retina) |

`packaging/dmg/background.tiff` is the combined file: both pictures in one HiDPI TIFF (made with `tiffutil -cathidpicheck`), so Finder picks the right one for the screen.

The PNGs hold only the paper, the arrow (a gray line that runs through the vermilion wave) and the line
“Drag EasyVox to Applications”. Finder draws the app icon, the Applications alias and the file names on top.

## Finder window settings

| Setting | Value |
|---|---|
| Window content size | 660 × 400 pt, the same as the picture |
| View | Icon view. Toolbar, sidebar, path bar and status bar hidden |
| Icon size | 128 |
| Text size | 13 |
| Label position | Bottom |
| Arrange by | None |
| `EasyVox.app` position (icon center) | 170, 164 |
| `Applications` alias position (icon center) | 490, 164 |
| Background | `background.tiff`, stored on the volume as the hidden file `.background.tiff` |
| Volume name | EasyVox |
| Disk image | `EasyVox-<version>.dmg`, from the crate version |

`packaging/dmg/settings.py` holds these settings for [dmgbuild](https://github.com/dmgbuild/dmgbuild), and `scripts/release.sh` builds the DMG with it.

## Notes

- **Only a light picture ships.** A DMG background can’t switch with the system appearance, and with a background
  picture set, Finder draws the file names in black and keeps the icon area light, even in Dark Mode. On a dark
  picture the labels would drop to about 1.2:1. The light picture keeps them at about 19:1 and looks right on both desktops.
- Contrast on the palest part of the paper (#F5F5F7): instruction text #6E6E73 4.7:1, arrow line #858589
  3.4:1, wave #F0462D 3.4:1.
- Finder may count the title bar in the window's height. If the bottom of the picture is cut off, add about 28 pt to
  `window_rect` in `packaging/dmg/settings.py`. Check the finished DMG on macOS 26, in both appearances, before a release.
- The instruction text is set in Inter and drawn into the picture.
