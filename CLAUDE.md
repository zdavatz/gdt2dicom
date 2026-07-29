# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

gdt2dicom converts between GDT files (German "Gerätedatentransfer" format used by
medical practice-management systems) and DICOM files/worklists, plus VDDS (dental)
and OPP (Open Practice Protocol) XML outputs. It is a Rust crate that builds several
CLI binaries and one GTK4 GUI app, distributed for Linux, Windows, and macOS.

**Core design: this project does not implement DICOM itself.** All DICOM work is
shelled out to dcmtk command-line tools (`img2dcm`, `xml2dcm`, `dcm2xml`, `dcmdump`,
`dump2dcm`, `dcmodify`, `dcmj2pnm`, `wlmscpfs`, `storescp`). dcmtk must be installed
for the binaries to work at runtime (`brew install dcmtk` on macOS).

## Build commands

```sh
cargo build                                        # all CLI binaries
cargo build --bin gdt2dicom-gui --features=gui     # GUI (requires GTK4 installed: brew install gtk4 / apt install libgtk-4-dev)
cargo fmt --check                                  # enforced by CI — run before pushing
```

There are no tests in this codebase (`test = false` on the bins, no `#[test]` anywhere).
CI (`.github/workflows/build.yml`) builds all binaries on Linux/Windows/macOS and fails
on `cargo fmt --check`.

Example run (sample data lives in `gdt/epat.gdt` and `wl/epat.wl`, untracked):

```sh
./target/debug/gdt2dicom --gdt-file gdt/epat.gdt --output out.wl
```

## Binaries

- `gdt2dicom` (`src/main.rs`, default-run) — GDT + optional JPEG folder → DICOM via
  `img2dcm`, or → DICOM worklist. **Mode is chosen by output file extension:** `.wl`
  produces a worklist (JPEGs ignored), anything else a DICOM file.
- `dicom2gdt` — DICOM → GDT, optionally exporting embedded images.
- `gdt2opp` — GDT → OPP XML.
- `gdt2vdds` — GDT → VDDS-media 1.x dental interface (INI-file handshake with
  installed dental imaging programs, see `src/vdds.rs`).
- `gdt2dicom-gui` — GTK4 app (feature-gated behind `gui`), the main end-user product.
- `testargs` — trivial debugging helper that dumps argv.

## Architecture

`src/lib.rs` exposes the shared library; the `gui` module is behind the `gui` feature
so CLI builds don't need GTK.

Data flow for conversions: `src/gdt.rs` parses GDT's line-based field-number format
into `GdtFile` (field numbers and German spec names are documented as comments on each
struct field). `src/dcm_xml.rs` is the bridge — it renders a `GdtFile` into dcmtk's
XML representation (as a `Vec<XmlEvent>` merged with either a user-supplied XML or a
built-in default template) and, in the reverse direction, extracts patient fields from
`dcm2xml` output. `src/dcm_worklist.rs` turns that XML into a `.wl` file via the
pipeline `xml2dcm → dcmdump → dump2dcm -g`, with `dcmodify` used to set AETitle/Modality.

`src/command.rs` wraps all subprocess execution. `binary_to_path()` is the important
piece: it resolves dcmtk binary names per-OS to match how releases are packaged —
Windows looks in `bin\` next to the exe, macOS looks in the app bundle's
`Contents/Resources/bin` then Homebrew/system paths (see the packaging steps in
`.github/workflows/build.yml`, which bundle dcmtk exactly there). On Windows every
`Command` needs the `CREATE_NO_WINDOW` creation flag to avoid console flashes — use
`new_command()`/`exec_command()` rather than raw `std::process::Command`. Logging goes
either to stdout or through an `mpsc::Sender<String>` so the GUI can show it.

`src/worklist_conversion.rs` is the auto-convert engine used by the GUI: it watches an
input folder (`notify` crate) for new `.gdt` files, converts each to `.wl` in the
shared worklist folder, and moves processed files into a `processed/` subfolder. It is
`Arc<Mutex<…>>`-shared with the filesystem-event handler and serializes to
`WorklistConversionState` for persistence.

The GUI (`src/bin/gdt2dicom-gui.rs` + `src/gui/`) is one window composed of sections:
a list of auto-convert folder watchers (`auto_convert_list.rs`/`auto_convert.rs`), a
worklist DICOM server section that runs dcmtk's `wlmscpfs` serving the worklist folder
(`dicom_server.rs`), and a C-STORE receiver running `storescp` that converts incoming
DICOMs to JPEG + GDT (`cstore_server.rs`). All UI state is persisted as `state.json`
**next to the executable** (`src/gui/state.rs`) — fields added to `StateFile` must be
`Option` for backward compatibility with existing users' files.

Non-ASCII handling: GDT parsing/writing works on raw bytes/UTF-8, but VDDS INI files
use the OS-local ANSI encoding (`local-encoding-ng`/`encoding` crates in `src/vdds.rs`).

## Releases

Pushing a tag triggers `.github/workflows/release.yml`. Packaging quirks live in
`build.yml`: Linux CLI binaries are statically linked (`-C target-feature=+crt-static`),
the Windows GUI zip bundles GTK DLLs plus dcmtk in `bin/`, and the macOS app is built
with `cargo-bundle` then patched by `bin/patch-dependencies.js` (rewrites dylib paths)
with dcmtk copied into `Contents/Resources/bin`.
