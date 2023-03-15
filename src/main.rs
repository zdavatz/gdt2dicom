use clap::Parser;

use std::ffi::OsStr;
use std::fs::{read_dir};
use std::io::{Write};
use std::path::{PathBuf};
use std::process::Command;
use tempfile::NamedTempFile;

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
    jpegs: String,

    #[arg(short, long)]
    output: String,
}

fn main() -> Result<(), std::io::Error> {
    let args = Args::parse();
    let jpegs = list_jpeg_files(args.jpegs)?;
    println!("Found Jpeg files: \n{}", jpegs.iter().map(|s| s.as_path().display().to_string()).collect::<Vec<String>>().join("\n"));
    let gdt_file = parse_file(args.gdt_file).unwrap();
    let xml = file_to_xml(gdt_file);
    let temp_file = NamedTempFile::new()?;
    std::fs::write(&temp_file, xml)?;

    let mut command_args = vec![OsStr::new("-nsc"), OsStr::new("-dx"), temp_file.path().as_os_str()];
    command_args.extend(jpegs.iter().map(|x| OsStr::new(x)));
    command_args.push(OsStr::new(&args.output));

    println!("Running: img2dcm {}", command_args.iter().map(|s| s.to_str().unwrap()).collect::<Vec<&str>>().join(" "));
    let output = Command::new("img2dcm")
        .args(command_args)
        .output()?;
    std::io::stdout().write_all(&output.stdout).unwrap();
    std::io::stderr().write_all(&output.stderr).unwrap();

    println!("Finished");
    return Ok(())
}

fn list_jpeg_files(dir_name: String) -> Result<Vec<PathBuf>, std::io::Error> {
    let mut entries = read_dir(dir_name)?
        .filter_map(|res| res.ok().map(|e| e.path()))
        .filter(|path|
            path.extension().and_then(|s| s.to_str()) == Some("jpg") ||
            path.extension().and_then(|s| s.to_str()) == Some("jpeg")
        )
        .collect::<Vec<_>>();
    entries.sort();
    return Ok(entries);
}
