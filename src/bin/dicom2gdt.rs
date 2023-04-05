use clap::Parser;

use std::path::PathBuf;

use gdt2dicom::dcm_xml::{export_images_from_dcm, parse_dcm_as_xml};
use gdt2dicom::gdt::{dcm_xml_to_file, file_to_string};

/// Convert a gdt file and an image folder to a dicom file
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    dicom_file: PathBuf,

    /// Where to output the GDT file, omitted = stdout
    #[arg(short, long)]
    gdt_file: Option<PathBuf>,

    /// Where to output the JPEG files
    #[arg(short, long)]
    jpegs: Option<PathBuf>,
}

fn main() -> Result<(), std::io::Error> {
    let args = Args::parse();
    if let Some(jpegs_path) = args.jpegs {
        println!("Exporting images to {}", &jpegs_path.display());
        export_images_from_dcm(&args.dicom_file, &jpegs_path).unwrap();
        println!("Exported images");
    }
    let events = parse_dcm_as_xml(&args.dicom_file).unwrap();
    let file = dcm_xml_to_file(&events);
    let gdt_string = file_to_string(file);

    if let Some(path) = args.gdt_file {
        std::fs::write(&path, gdt_string)?;
        println!("GDT File written to:{}", path.display());
    } else {
        println!("GDT File:\r\n{}", &gdt_string);
    }

    return Ok(());
}
