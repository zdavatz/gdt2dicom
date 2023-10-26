use clap::Parser;

use std::path::PathBuf;

use gdt2dicom::gdt::parse_file;

use gdt2dicom::opp_xml::file_to_xml;

/// Convert a gdt file to opp xml with patient info
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    gdt_file: PathBuf,

    #[arg(short, long)]
    output: PathBuf,
}

fn main() -> Result<(), std::io::Error> {
    let args = Args::parse();

    let gdt_file = parse_file(args.gdt_file).unwrap();
    file_to_xml(gdt_file, args.output.clone()).unwrap();

    println!("Finished, output at {}", args.output.display());
    return Ok(());
}
