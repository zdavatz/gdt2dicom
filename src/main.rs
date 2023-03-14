use clap::Parser;

pub mod gdt;
pub mod dcm_xml;

use crate::gdt::{ parse_file };
use crate::dcm_xml::{ file_to_xml };

/// Convert a gdt file and an image folder to a dicom file
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    gdt_file: String,

    #[arg(short, long)]
    jpegs: Option<String>,
}

fn main() {
    let args = Args::parse();
    let file = parse_file(args.gdt_file).unwrap();
    dbg!(&file);
    let xml = file_to_xml(file);
    println!("{}", xml);
    println!("Finished");
}
