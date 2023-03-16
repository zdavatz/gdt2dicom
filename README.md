# gdt2dicom
Convert a gdt file and an image folder to a dicom file


## Build

* Install `dcmtk`, that will give you `img2dcm`
* Install Rust: https://www.rust-lang.org/tools/install

```
cargo build
```

The binary should be built at `./target/debug/gdt2dicom`

You can run the binary like this:

```
./target/debug/gdt2dicom --gdt-file <GDT_FILE> --jpegs <FOLDER CONTAINING JPEGS> --output <OUTPUT DCM PATH>
```
