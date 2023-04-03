use clap::Parser;

// use std::ffi::OsStr;
// use std::fs::read_dir;
use std::path::PathBuf;

use gdt2dicom::dcm_xml::{export_images_from_dcm, parse_dcm_as_xml};
use gdt2dicom::gdt::dcm_xml_to_file;

/// Convert a gdt file and an image folder to a dicom file
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    gdt_file: PathBuf,

    #[arg(short, long)]
    dicom_file: PathBuf,

    #[arg(short, long)]
    jpegs: Option<PathBuf>,
}

fn main() -> Result<(), std::io::Error> {
    let args = Args::parse();
    dbg!(&args);
    if let Some(jpegs_path) = args.jpegs {
        println!("Exporting images to {}", &jpegs_path.display());
        export_images_from_dcm(&args.dicom_file, &jpegs_path).unwrap();
        println!("Exported images");
    }
    let events = parse_dcm_as_xml(&args.dicom_file).unwrap();
    let file = dcm_xml_to_file(&events);
    dbg!(&file);
    return Ok(());
}
