# AJS PDF Editor (APE) app icon

<img src="pdfcraft-small.svg" alt="APE app icon: a white document with a pink-red PDF band and a blue pencil on a charcoal tile" width="128">

AJS logo for APE, option A (approved 2026-11-19): a white page with a folded corner, the AJS pink-red band
lettered "PDF" (outlined paths, no font), grey text lines and a blue pencil for editing, on a charcoal
rounded tile. Sizes of 32 px and below use a simpler drawing of the same shape without lettering.

**Palette:** charcoal `#3B4453`, white `#FFFFFF`, light grey `#C9D1DC`, sky blue `#29ABE2`, AJS pink-red `#E51050`.

**Tile:** `viewBox="0 0 512 512"`, a 480 px square with `rx=96`; the small drawing uses `viewBox="0 0 32 32"`.

**Provenance:** drawn by AJS (autolisp.vn); the source and the script that renders every size
(`make-APE.ps1`) live in `03.AutoDesk Store/_Branding/APE` of the AJS plugins repository.
It replaces the PdfCraft lion icon; PdfCraft remains credited in the licences and the About window.
Licence: [LICENSE.txt](LICENSE.txt) (`MIT OR Apache-2.0`, like the repo).

## Files

| File | What it is |
|---|---|
| `pdfcraft.svg` | the master vector (APE.svg); every PNG and the `.ico` are rendered from it |
| `pdfcraft-small.svg` | the small-size vector (APE-small.svg), used for 16–32 px |
| `pdfcraft-1024.png` | 1024 px (APE-1080.png scaled down); also the runtime Dock icon on macOS |
| `pdfcraft.icns` | macOS icon (128–1024 px, PNG entries) |
| `pdfcraft.ico` | Windows icon (16–256 px), embedded in `pdfcraft.exe` by `apps/pdfcraft/build.rs` |
| `pdfcraft-document.svg` | Windows PDF file icon master: a Lucide document outline, red badge and outlined PDF lettering; no font dependency |
| `pdfcraft-document.ico` | Windows PDF file icon (16–256 px), embedded as resource 2 in `pdfcraft.exe` |
| `hicolor/<n>x<n>/apps/ai.storyteller.pdfcraft.png` | Linux hicolor theme, 16–512 px; the 256 px one is the runtime icon on Windows and Linux |
| `hicolor/scalable/apps/ai.storyteller.pdfcraft.svg` | Linux scalable icon (copy of the master) |

Where it shows: `apps/pdfcraft/src/main.rs` sets the window icon (Dock, taskbar, Alt-Tab, launcher) and the
Wayland app id `ai.storyteller.pdfcraft`; `packaging/linux/ai.storyteller.pdfcraft.desktop` names the
hicolor icon.

Windows PDF files use resource 2 (`"pdfcraft.exe",-2`) when PdfCraft is chosen as the default
reader; the app keeps resource 1. The document outline is adapted from Lucide `file-text`
1.49.0 (ISC); its badge and outlined lettering are original (`MIT OR Apache-2.0`), covered by
[LICENSE-document.txt](LICENSE-document.txt). Both icons are embedded, requiring no runtime
sidecar. This provides a file type icon; first-page thumbnails still need a thumbnail provider.

## Regenerate

Run `make-APE.ps1 -Preview -Export` in `_Branding/APE` (Windows PowerShell 5.1), copy the outputs over the
files above, then update the sha256 values in `ATTRIBUTION.toml` and run `cargo xtask assets --write`.
