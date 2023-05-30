# gdt2dicom
Convert a gdt file and an image folder to a dicom file


## Build

* Install `dcmtk`, that will give you `img2dcm`
* Install Rust: https://www.rust-lang.org/tools/install

```
cargo build
```

The binary should be built at `./target/debug/gdt2dicom` and `./target/debug/dicom2gdt`

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

You can convert a GDT file to a Worklist file doing

```
gdt2dicom --gdt-file epat.get --output epat.wl
```
