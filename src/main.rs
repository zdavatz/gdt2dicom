use clap::Parser;

use std::ffi::OsStr;
use std::fs::read_dir;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

pub mod dcm_xml;
pub mod gdt;

use crate::dcm_xml::{default_dcm_xml, file_to_xml, parse_dcm_xml};
use crate::gdt::parse_file;

/// Convert a gdt file and an image folder to a dicom file
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    gdt_file: PathBuf,

    #[arg(short, long)]
    dicom_xml: Option<PathBuf>,

    #[arg(short, long)]
    jpegs: PathBuf,

    #[arg(short, long)]
    output: PathBuf,
}

fn main() -> Result<(), std::io::Error> {
    let args = Args::parse();
    let jpegs = list_jpeg_files(&args.jpegs)?;
    println!(
        "Found Jpeg files: \n{}",
        jpegs
            .iter()
            .map(|s| s.as_path().display().to_string())
            .collect::<Vec<String>>()
            .join("\n")
    );

    let dicom_xml_path = match args.dicom_xml {
        Some(p) => Some(PathBuf::from(p)),
        None => find_xml_path(&args.jpegs)?,
    };
    println!(
        "Dicom XML file path: \n{}",
        dicom_xml_path
            .clone()
            .and_then(|p| p.into_os_string().into_string().ok())
            .unwrap_or("None".to_string())
    );

    let xml_events = match dicom_xml_path {
        Some(p) => parse_dcm_xml(&p).expect("Expecting a good xml file."),
        _ => default_dcm_xml(),
    };

    let gdt_file = parse_file(args.gdt_file).unwrap();
    let temp_file = file_to_xml(gdt_file, &xml_events).unwrap();

    let mut command_args = vec![
        OsStr::new("-nsc"),
        OsStr::new("-dx"),
        temp_file.path().as_os_str(),
    ];
    command_args.extend(jpegs.iter().map(|x| OsStr::new(x)));
    command_args.push(OsStr::new(&args.output));

    println!(
        "Running: img2dcm {}",
        command_args
            .iter()
            .map(|s| s.to_str().unwrap())
            .collect::<Vec<&str>>()
            .join(" ")
    );
    let output = Command::new("img2dcm").args(command_args).output()?;
    std::io::stdout().write_all(&output.stdout).unwrap();
    std::io::stderr().write_all(&output.stderr).unwrap();

    println!("Finished");
    return Ok(());
}

fn list_jpeg_files(dir_name: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
    let mut entries = read_dir(dir_name)?
        .filter_map(|res| res.ok().map(|e| e.path()))
        .filter(|path| {
            path.extension().and_then(|s| s.to_str()) == Some("jpg")
                || path.extension().and_then(|s| s.to_str()) == Some("jpeg")
        })
        .collect::<Vec<_>>();
    entries.sort();
    return Ok(entries);
}

fn find_xml_path(dir_name: &Path) -> Result<Option<PathBuf>, std::io::Error> {
    let xml_file = read_dir(dir_name)?
        .filter_map(|res| res.ok())
        .find(|dir| dir.path().extension().and_then(|s| s.to_str()) == Some("xml"));
    return Ok(xml_file.map(|x| x.path()));
}
