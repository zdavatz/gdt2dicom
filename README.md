# gdt2dicom

Convert between GDT files (the German „Gerätedatentransfer" practice-management
exchange format) and DICOM files / DICOM worklists. Can also export to VDDS-media
(dental imaging) and Open Practice Protocol (OPP) XML.

## Build

* Install [dcmtk](https://dicom.offis.de/dcmtk/) — all DICOM work is delegated to
  its command line tools (`img2dcm`, `xml2dcm`, `dcmdump`, `dump2dcm`, `dcmodify`, …),
  so they must be available at runtime. On macOS: `brew install dcmtk`.
* Install Rust: https://www.rust-lang.org/tools/install

```
cargo build
```

The binaries are built at `./target/debug/`: `gdt2dicom`, `dicom2gdt`, `gdt2opp`
and `gdt2vdds`.

### GUI

There is also a GTK4 GUI app that watches folders and auto-converts incoming GDT
files to worklist files, serves the worklist folder with dcmtk's `wlmscpfs`, and
receives images via C-STORE (`storescp`), converting them back to JPEG + GDT.

* Install GTK4: `brew install gtk4` (macOS) or `apt install libgtk-4-dev` (Linux)

```
cargo build --bin gdt2dicom-gui --features=gui
```

## Usage

### GDT to Dicom

You can run the binary like this:

```
./target/debug/gdt2dicom --gdt-file <GDT_FILE> --jpegs <FOLDER CONTAINING JPEGS> --output <OUTPUT DCM PATH>
```

By default it searches for a xml file for Dicom in the jpegs folder (#5),
if it cannot find one it uses a default file. You can also specify the xml file with the `-d` or `--dicom-xml` flag.

### Dicom to GDT

You can convert Dicom 2 GDT as well.

```
./target/debug/dicom2gdt --dicom-file <DCM FILE> [--gdt-file <GDT FILE>] [--jpegs <PATH TO JPEGS>]
```

- `--gdt-file` or `-g` is optional, when it's not present, it will be outputed to stdout.
- `--jpegs` or `-j` is optional, when it's not present, images will not be exported.

### GDT zu Worklist file

You can convert a GDT file to a Worklist file. The mode is selected by the output
extension: `.wl` produces a worklist instead of a Dicom file.

```
./target/debug/gdt2dicom --gdt-file epat.gdt --output epat.wl
```

### GDT to Open Practice Protocol

```
./target/debug/gdt2opp --gdt-file <GDT FILE> --output opp.xml
```

### GDT to VDDS

Sends the patient from a GDT file to a dental imaging program (BVS) registered in
`VDDS_MMI.ini`, and saves the images it returns into the output folder.

```
./target/debug/gdt2vdds --gdt-file <GDT FILE> --output <IMAGE OUTPUT FOLDER> [--ext JPG|TIF|PNG|DCM] [--bvs <BVS NAME>] [--vdds-mmi <PATH TO VDDS_MMI.ini>]
```

## Releases

Pushing a git tag builds and publishes release artifacts for Linux, Windows and
macOS via GitHub Actions (`.github/workflows/release.yml`). The Windows and macOS
GUI bundles ship with the required dcmtk binaries included.
