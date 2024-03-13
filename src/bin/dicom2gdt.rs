use clap::Parser;

use std::fs::create_dir_all;
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

    /// Where to output the PNG files
    #[arg(short, long)]
    pngs: Option<PathBuf>,
}

fn main() -> Result<(), std::io::Error> {
    let args = Args::parse();
    let events = parse_dcm_as_xml(&args.dicom_file).unwrap();
    let file = dcm_xml_to_file(&events);
    if let Some(pngs_path) = args.pngs {
        if !pngs_path.exists() {
            println!(
                "Path {} doesn't exist, creating directory...",
                pngs_path.display()
            );
            create_dir_all(&pngs_path)?;
            println!("Created {}", pngs_path.display());
        }
        if !pngs_path.is_dir() {
            println!(
                "{} is not a directory, not exporting png.",
                pngs_path.display()
            )
        } else {
            let mut png_with_prefix = pngs_path.clone();
            png_with_prefix.push(file.object_patient.patient_number.clone());
            println!("Exporting images to {}", &png_with_prefix.display());
            export_images_from_dcm(&args.dicom_file, &png_with_prefix).unwrap();
            println!("Exported images");
        }
    }
    let gdt_string = file_to_string(file);

    if let Some(path) = args.gdt_file {
        std::fs::write(&path, gdt_string)?;
        println!("GDT File written to:{}", path.display());
    } else {
        println!("GDT File:\r\n{}", &gdt_string);
    }

    return Ok(());
}
