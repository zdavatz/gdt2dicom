use clap::Parser;
use env_logger::Env;
use ini::Ini;
use log::{debug, error, info};

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use gdt2dicom::gdt::parse_file;
use gdt2dicom::vdds;

/// Convert a gdt file to opp xml with patient info
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    gdt_file: PathBuf,

    #[arg(long)]
    vdds_mmi: Option<PathBuf>,

    #[arg(long)]
    bvs: Option<String>,

    #[arg(short, long)]
    output: PathBuf,
}

fn main() -> Result<(), std::io::Error> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    let args = Args::parse();

    let output_attr = fs::metadata(&args.output);
    match output_attr {
        Err(err) => {
            error!("Output has to be a folder");
            error!("{}", err);
            std::process::exit(1);
        }
        Ok(attr) if !attr.is_dir() => {
            error!("Output has to be a folder");
            std::process::exit(1);
        }
        _ => {}
    }

    let vdds_mmi_path = &args.vdds_mmi.unwrap_or_else(vdds::default_vdds_mmi_folder);
    info!("Loading VDDS_MMI: {}", vdds_mmi_path.display());
    let mut mmi = Ini::load_from_file(vdds_mmi_path).unwrap();

    let pvs_section = mmi.section_mut(Some("PVS")).unwrap();
    let pvs_count = pvs_section.len();
    let is_inserted_to_vdds_mmi = pvs_section
        .iter()
        .any(|(_key, value)| value == vdds::PVS_NAME);
    debug!(
        "is {} in VDDS_MMI? {}",
        vdds::PVS_NAME,
        is_inserted_to_vdds_mmi
    );
    if !is_inserted_to_vdds_mmi {
        let proposed_name = (1..=pvs_count + 1).find_map(|i| {
            let name = format!("NAME{}", i);
            if pvs_section.get(&name).is_none() {
                Some(name)
            } else {
                None
            }
        });
        debug!("Inserting {} to {:?}", vdds::PVS_NAME, proposed_name);

        match proposed_name {
            None => error!("Cannot insert {} into VDDS_MMI", vdds::PVS_NAME),
            Some(name) => {
                pvs_section.append(name, vdds::PVS_NAME);

                let current_path = std::env::current_exe()?;
                mmi.with_section(Some(vdds::PVS_NAME))
                    .set("MMOINFIMPORT", current_path.to_string_lossy())
                    .set("MMOINFIMPORT_OS", vdds::vdds_os())
                    .set("NAME", "gdt2dicom")
                    .set("STAGES", "1234")
                    .set("VERSION", "1.0");
            }
        }
        info!("Updating VDDS_MMI");
        mmi.write_to_file(vdds_mmi_path).unwrap();
    }

    let bvs = mmi.section(Some("BVS")).expect("BVS Section in VDDS_MMI");
    if bvs.len() == 0 {
        error!("No BVS Found");
        std::process::exit(1);
    }
    let bvs_name = if bvs.len() == 1 {
        let (_key, name) = bvs.iter().next().expect("First BVS");
        name
    } else {
        match &args.bvs {
            Some(preferred_bvs) => {
                let has_bvs = bvs.iter().any(|(_key, name)| name == preferred_bvs);
                if !has_bvs {
                    error!("Cannot find the specified BVS, please choose from one of these:");
                    for (_key, name) in bvs.iter() {
                        error!("- {}", name);
                    }
                    std::process::exit(2)
                } else {
                    preferred_bvs
                }
            }
            None => {
                error!("Multiple BVS available, please specify one of the following with --bvs");
                for (_key, name) in bvs.iter() {
                    error!("- {}", name);
                }
                std::process::exit(3)
            }
        }
    };
    info!("Sending to BVS: {}", bvs_name);

    let bsv_section = mmi.section(Some(bvs_name)).expect("BVS-named Section");
    let patient_import_exe = bsv_section
        .get("PATDATIMPORT")
        .expect("PATDATIMPORT in BVS");

    let gdt_file = parse_file(&args.gdt_file).unwrap();
    let patient_vdds_file = vdds::VddsPatient::new(&gdt_file);

    info!("Sending PATDATIMPORT");
    let _ = patient_vdds_file.send_vdds_file(patient_import_exe.to_string(), bvs_name.to_string());

    let info_export_exe = bsv_section
        .get("MMOINFEXPORT")
        .expect("MMOINFEXPORT in BVS");
    info!("Sending MMOINFEXPORT");
    let vdds_inf_export_req = vdds::ImageInfoRequest {
        pat_id: gdt_file.object_patient.patient_number.clone(),
    };
    let mmo_infos =
        vdds_inf_export_req.send_vdds_file(info_export_exe.to_string(), bvs_name.to_string())?;
    debug!("MMO Infos: {:?}", mmo_infos);
    return Ok(());
}
