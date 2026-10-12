# dmgbuild settings for EasyVox-<version>.dmg. scripts/release.sh runs it from the repo root:
#
#   uvx --python 3.12 --with ds_store==1.3.3 --with mac_alias==2.2.3 dmgbuild==1.6.7 \
#     -s packaging/dmg/settings.py -D app=dist/EasyVox.app EasyVox dist/EasyVox-0.1.0.dmg
#
# dmgbuild writes the Finder window's layout (.DS_Store) itself, so it needs no Finder and works
# over SSH and in CI. It runs this file with exec(); `defines` holds the -D values. Paths are
# relative to the repo root. The window settings come from the design hand-off
# (design/assets/dmg/README.md).

app = defines["app"]  # noqa: F821 (dmgbuild provides `defines`)

format = "ULFO"  # LZFSE: quick to open, and readable on every macOS EasyVox runs on
files = [app]
symlinks = {"Applications": "/Applications"}
# No hide_extensions: Finder already hides an app's ".app", and the flag would put a
# com.apple.FinderInfo attribute on the bundle, which fails `codesign --verify --strict`.

# The HiDPI TIFF (660 × 400 pt, with a 2× image). dmgbuild copies it to the volume as the hidden
# file .background.tiff and points the window at it.
background = "packaging/dmg/background.tiff"

# Origin (200, 120) and size 660 × 400 pt, the picture's size. Finder may count the title bar in
# this height: if the bottom of the picture is cut off, add about 28 pt.
window_rect = ((200, 120), (660, 400))
default_view = "icon-view"
show_status_bar = False
show_tab_view = False
show_toolbar = False
show_pathbar = False
show_sidebar = False

arrange_by = None
label_pos = "bottom"
icon_size = 128
text_size = 13
# Icon centers, in points from the window content's top left.
icon_locations = {
    "EasyVox.app": (170, 164),
    "Applications": (490, 164),
}
