use clap::Parser;

// use std::ffi::OsStr;
// use std::fs::read_dir;
// use std::io::Write;
use std::path::{PathBuf};
// use std::process::Command;

// pub mod dcm_xml;
// pub mod gdt;

// use crate::dcm_xml::{default_dcm_xml, file_to_xml, parse_dcm_xml};
// use crate::gdt::parse_file;

/// Convert a gdt file and an image folder to a dicom file
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    gdt_file: PathBuf,

    #[arg(short, long)]
    dicom: PathBuf,

    #[arg(short, long)]
    jpegs: PathBuf,

    #[arg(short, long)]
    output: PathBuf,
}

fn main() -> Result<(), std::io::Error> {

    let args = Args::parse();
    dbg!(args);
    return Ok(());
}
