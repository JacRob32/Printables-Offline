# Printables Offline

A desktop app for downloading and managing 3D printing models from Printables.com. Clone models to your local library, browse them offline, and open files directly in your slicer.

<!-- Replace this with a screenshot of the app -->
<!-- [screenshot.png](screenshot.png) -->

## Features

- **Clone models** — Paste a Printables.com URL and the app downloads the model files, cover image, tags, and metadata to your local library.
- **Local library** — Browse all cloned models in a grid view with cover images, file counts, and sizes. Search and filter by file type.
- **Collections** — Group models into custom collections, browsable from the sidebar. Add or remove models from a collection right on the model's page, and rename or delete collections as your library grows.
- **Detail view** — View model description, tags, file list, and source URL for each model.
- **Open in slicer** — Launch your configured slicer (PrusaSlicer, OrcaSlicer, Bambu Studio, Cura) with model files directly from the app.
- **Export** — Copy model files to any destination folder.
- **Delete** — Remove models from your library.
- **Settings** — Configure library folder, slicer executable, and theme (light/dark/system). Preferences persist across sessions.

## How It Works

The app uses a Python scraper (bundled in `py/`) that calls the Printables GraphQL API to fetch model metadata, generate download links, and retrieve cover images. The Rust/Tauri backend manages the UI, file system operations, and launches external processes.

All data is stored locally. No account or API key is required.

## Requirements

- macOS (Apple Silicon or Intel) or Windows 10/11 (x64 or ARM64)
- Python 3.9+ with `cloudscraper`, `beautifulsoup4`, and `requests` installed
- A slicer application (optional, for the "Open in Slicer" feature)

## Install

### macOS

Download the latest `.dmg` from [Releases](#releases), open it, and drag **Printables Offline.app** to your Applications folder.

On first launch, go to **Preferences** and set your library folder. This is where cloned models will be stored.

### Windows

Download the latest `.exe` for your architecture (x64 or ARM64) from [Releases](#releases). This is an early, unpackaged build: place it in its own folder alongside a copy of this repo's `py/` folder, then run the `.exe` directly. There is no installer yet.

## macOS Installation Guide

Because this application is open-source and not signed with a paid Apple Developer Account, macOS Gatekeeper will initially block it from running. To allow it:

1. Try to open **Printables Offline.app**. Gatekeeper will block it with a warning.
2. Open **System Settings** → **Privacy & Security**.
3. Scroll down to the security notice about Printables Offline and click **Open Anyway**.
4. Confirm **Open** on the dialog that appears.

You only need to do this once, on first launch.

## Build from Source

```bash
# Clone the repo
git clone https://github.com/JacRob32/Printables-Offline.git
cd Printables-Offline

# Install Python dependencies
pip3 install cloudscraper beautifulsoup4 requests

# Build the app
cd src-tauri
cargo tauri build
```

The `.app` and `.dmg` will be in `src-tauri/target/release/bundle/`.

For development with hot reload:

```bash
cargo tauri dev
```

### Cross-compiling for Windows

Windows builds are produced from macOS using [`cargo-xwin`](https://github.com/rust-cross/cargo-xwin), which downloads the MSVC CRT and Windows SDK on demand:

```bash
brew install llvm lld
rustup target add x86_64-pc-windows-msvc aarch64-pc-windows-msvc
cargo install cargo-xwin

export PATH="/opt/homebrew/opt/llvm/bin:/opt/homebrew/opt/lld/bin:$PATH"
cd src-tauri
cargo xwin build --release --target x86_64-pc-windows-msvc
cargo xwin build --release --target aarch64-pc-windows-msvc
```

The resulting `.exe` files are in `src-tauri/target/<target-triple>/release/`.

## Settings Storage

Preferences are saved to `~/.printablesoffline/prefs.json`. You can edit this file directly if needed.

## Platform Support

macOS (Apple Silicon or Intel) and early Windows builds (x64 and ARM64), cross-compiled from macOS using `cargo-xwin`. Windows builds are not yet packaged as installers and require the `py/` folder to sit alongside the `.exe`.

## Releases

| Version | Date | Notes |
|---------|------|-------|
| 1.1.0 | 2026-09-13 | Collections feature; fixed collection create/rename and default slicer path; fixed library folder/storage not reflecting in Settings; added Windows x64/ARM64 builds. |
| 1.0.1 | 2026-09-10 | Cover images now use the first downloaded picture in the library. |
| 1.0.0 | 2026-08-24 | Initial release. Model cloning, local library, slicer integration, settings persistence. |

Download: [Latest release](https://github.com/JacRob32/Printables-Offline/releases)

## License

MIT
