use clap::Parser;

pub mod gdt;

use crate::gdt::{ parse_file };

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
    let iter = parse_file(args.gdt_file).unwrap();

    for line in iter {
        dbg!(line);
    }
    println!("Finished");
}
