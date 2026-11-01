# iPad and Apple Pencil

SolidCraft runs on an iPad (and other touch screens) in the browser: the same app as on the
desktop, built for the web. It needs iPadOS with WebGPU or WebGL 2 in Safari; an iPad Pro (4th
generation, 2020) or later with an Apple Pencil (2nd generation) works well.

## Install it like an app

1. Open the SolidCraft web app in Safari.
2. Tap **Share › Add to Home Screen**.
3. Start SolidCraft from its Home Screen icon. It opens full screen, without Safari's bars, and
   keeps working offline.

A Magic Keyboard or any Bluetooth keyboard and trackpad work as on a computer: shortcuts, Shift
and the trackpad's gestures all apply.

## Fingers and the Pencil

| On the model | Does |
|---|---|
| Apple Pencil | The mouse's left button: pick, draw sketches, drag geometry, box select, press buttons |
| One finger, drag | Turn the view (on a flat page such as a drawing: pan) |
| Two fingers, move | Pan: the model follows the fingers |
| Two fingers, pinch | Zoom, toward the point between the fingers (Preferences › Navigation › Zoom to the cursor) |
| One finger, tap | A click: pick, place a point, press a button on the model |
| One finger, hold still | The context menu (as a right click) |

On panels, windows and menus a finger works like the mouse, so lists scroll and sliders drag.
While the Pencil touches the screen, fingers do nothing, so a palm resting on the glass doesn't
move the view or pick anything. With a keyboard attached, Shift and a Pencil tap add to the
selection as Shift+click does.

To use fingers as the mouse everywhere instead (the behaviour before this was added), turn off
**Preferences › Navigation › Finger gestures on touch screens**.

## OneDrive, Google Drive and iCloud Drive

The iPad's Files app reaches every storage the iPad has: iCloud Drive, On My iPad, and the
OneDrive, Google Drive and Dropbox apps once they are installed and signed in. Turn each one on
in Files › Browse › ⋯ › Edit if it isn't listed.

- **Open from OneDrive or Google Drive:** File › Open shows the designs kept on this iPad
  ("Files in this browser"). Tap **Open from Files…**, choose Browse, then OneDrive or Google
  Drive, and pick the design (`.sc`). **Insert from Files…** brings in a STEP, STL, 3MF or other
  file the same way.
- **Save to OneDrive or Google Drive:** tap **Save to Files…** in that window (or File ›
  Download design, or any export). The share sheet opens: tap **Save to Files**, choose OneDrive
  or Google Drive and the folder, and tap Save. Safari cannot write back to a file it opened, so
  each save is a new copy: give it the same name to replace the old one.
- **Save** and **Save As** keep the design in the app's own storage on the iPad, listed in
  "Files in this browser", so work is never lost between visits; an autosave of unsaved work is
  kept there too.

The Apple Pencil's double tap and squeeze are not available to web apps, so they do nothing in
SolidCraft.
