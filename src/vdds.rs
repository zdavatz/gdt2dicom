use ini::Ini;
use log::{debug, error, info};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

use crate::command::exec_command;
use crate::gdt::{GdtFile, GdtPatientGender};

pub static PVS_NAME: &str = "gdt2dicom_PVS";

pub struct VddsPatient {
    id: String,
    first_name: String,
    last_name: String,
    title: String,
    date_of_birth: String, // YYYYMMDD
    sex: VddsPatientGender,
    street: String,
    city: String,
    phone: String,
    mobile_phone: String,
    email_address: String,
    insurance_number: String,
}

#[derive(Debug)]
pub enum VddsPatientGender {
    Male,
    Female,
}

impl VddsPatient {
    pub fn new(gdt_file: &GdtFile) -> VddsPatient {
        let p = &gdt_file.object_patient;
        VddsPatient {
            id: p.patient_number.clone(),
            first_name: p.patient_first_name.clone(),
            last_name: p.patient_name.clone(),
            title: p.patient_title.clone(),
            date_of_birth: gdt_date_to_vdds(&p.patient_dob),
            sex: gdt_gender_to_vdds(&p.patient_gender),
            street: p.street.clone(),
            city: p.address.clone(),
            phone: p.phone_number.clone(),
            mobile_phone: p.mobile_phone_number.clone(),
            email_address: p.email_address.clone(),
            insurance_number: p.insurance_number.clone(),
        }
    }
    pub fn to_ini(&self, bvs_name: String) -> Ini {
        let mut ini = Ini::new();
        let mut binding = ini.with_section(Some("PATIENT"));
        let mut section = binding
            .set("PVS", PVS_NAME)
            .set("BVS", bvs_name)
            .set("PATID", &self.id)
            .set("FIRSTNAME", &self.first_name)
            .set("LASTNAME", &self.last_name)
            .set(
                "SEX",
                match self.sex {
                    VddsPatientGender::Female => "W",
                    VddsPatientGender::Male => "M",
                },
            );

        // Optional fields
        if self.title.len() > 0 {
            section = section.set("TITLE", &self.title);
        }
        if self.date_of_birth.len() > 0 {
            section = section.set("BIRTHDAY", &self.date_of_birth);
        }
        if self.street.len() > 0 {
            section = section.set("STREET", &self.street);
        }
        if self.city.len() > 0 {
            section = section.set("CITY", &self.city);
        }
        if self.phone.len() > 0 {
            section = section.set("HOMEPHONE", &self.phone);
        }
        if self.mobile_phone.len() > 0 {
            section = section.set("CELLULAR", &self.mobile_phone);
        }
        if self.email_address.len() > 0 {
            section = section.set("EMAIL", &self.email_address);
        }
        if self.insurance_number.len() > 0 {
            section = section.set("INSURANCENUMBER", &self.insurance_number);
        }
        section = section
            .set("READY", "0")
            .set("ERRORLEVEL", "0")
            .set("ERRORTEXT", "");
        return ini;
    }

    pub fn send_vdds_file<P>(&self, exe_path: P, bvs_name: String) -> Result<Ini, std::io::Error>
    where
        P: Into<PathBuf>,
    {
        let ini_file = self.to_ini(bvs_name);

        let result = send_and_wait(exe_path, ini_file, Some("PATIENT".to_string()))?;

        Ok(result)
    }
}

        return ini;
    }

    pub fn send_vdds_file<P>(&self, exe_path: P) -> Result<(), std::io::Error>
    where
        P: Into<PathBuf>,
    {
        let ini_file = self.to_ini();
        let result = send_and_wait(exe_path, ini_file, Some("MMOIDS".to_string()))?;

        let section = result.section(Some("MMOPATH")).expect("MMOPATH in reply");
        let mut paths: HashMap<String, String> = HashMap::new();
        for (key, value) in section.iter() {
            paths.insert(key.to_string(), value.to_string());
        }
        Ok(paths)
    }
}

pub fn send_and_wait<P>(
    exe_path: P,
    ini: Ini,
    section_name: Option<String>,
) -> Result<Ini, std::io::Error>
where
    P: Into<PathBuf>,
{
    let temp_file = NamedTempFile::new()?;
    let temp_file_path = temp_file.path();
    ini.write_to_file(&temp_file_path)?;
    let path = exe_path.into();
    let path_str = path.to_string_lossy();
    info!("Sending ini to {:?}", &path_str);
    exec_command(&path_str, vec![temp_file_path], true)?;
    let result = wait_for_ready(temp_file_path, section_name);
    return Ok(result);
}

fn wait_for_ready(path: &Path, section_name: Option<String>) -> Ini {
    debug!("Waiting for response: {:?}", path);
    loop {
        let mmi = Ini::load_from_file(path).unwrap();
        let section = mmi.section(section_name.clone()).unwrap();
        let ready = section.get("READY");
        if ready == Some("1") {
            let error_level = section.get("ERRORLEVEL");
            let error_text = section.get("ERRORTEXT");

            if error_level == Some("0") {
                return mmi;
            }
            error!("Error from BVS: ({:?}): {:?}", error_level, error_text);
            std::process::exit(100);
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

fn gdt_date_to_vdds(str: &String) -> String {
    // DDMMYYYY -> YYYYMMDD
    if str.len() < 8 {
        return format!("{}", str);
    }
    let day = &str[0..2];
    let month = &str[2..4];
    let year = &str[4..8];
    return format!("{}{}{}", year, month, day);
}

fn gdt_gender_to_vdds(gender: &GdtPatientGender) -> VddsPatientGender {
    match gender {
        crate::gdt::GdtPatientGender::Male => VddsPatientGender::Male,
        crate::gdt::GdtPatientGender::Female => VddsPatientGender::Female,
    }
}

#[cfg(target_os = "macos")]
pub fn default_vdds_mmi_folder() -> PathBuf {
    "/etc/vdds/VDDS_MMI.INI".into()
}
#[cfg(target_os = "windows")]
pub fn default_vdds_mmi_folder() -> PathBuf {
    "C:\\Windows\\VDDS_MMI.INI".into()
}
#[cfg(target_os = "linux")]
pub fn default_vdds_mmi_folder() -> PathBuf {
    "/etc/vdds/VDDS_MMI.INI".into()
}

#[cfg(target_os = "macos")]
pub fn vdds_os() -> String {
    "3".to_string()
}
#[cfg(target_os = "windows")]
pub fn vdds_os() -> String {
    "1".to_string()
}
#[cfg(target_os = "linux")]
pub fn vdds_os() -> String {
    "3".to_string()
}
