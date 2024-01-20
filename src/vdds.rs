use ini::Ini;
use std::path::PathBuf;

use crate::gdt::{GdtFile, GdtPatientGender};

pub struct VddsFile {
    patient: VddsPatient,
}

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

pub fn from_gdt(gdt_file: &GdtFile) -> VddsFile {
    let p = &gdt_file.object_patient;
    VddsFile {
        patient: VddsPatient {
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
}

pub fn to_ini(file: &VddsFile) -> Ini {
    let p = &file.patient;
    let mut ini = Ini::new();
    let mut binding = ini.with_section(Some("PATIENT"));
    let mut section = binding
        .set("PVS", "THE_PVS")
        .set("BVS", "THE_BVS")
        .set("PATID", &p.id)
        .set("FIRSTNAME", &p.first_name)
        .set("LASTNAME", &p.last_name)
        .set("SEX", match p.sex { VddsPatientGender::Female => "W", VddsPatientGender::Male => "M", });

    // Optional fields
    if p.title.len() > 0 {
        section = section.set("TITLE", &p.title);
    }
    if p.date_of_birth.len() > 0 {
        section = section.set("BIRTHDAY", &p.date_of_birth);
    }
    if p.street.len() > 0 {
        section = section.set("STREET", &p.street);
    }
    if p.city.len() > 0 {
        section = section.set("CITY", &p.city);
    }
    if p.phone.len() > 0 {
        section = section.set("HOMEPHONE", &p.phone);
    }
    if p.mobile_phone.len() > 0 {
        section = section.set("CELLULAR", &p.mobile_phone);
    }
    if p.email_address.len() > 0 {
        section = section.set("EMAIL", &p.email_address);
    }
    if p.insurance_number.len() > 0 {
        section = section.set("INSURANCENUMBER", &p.insurance_number);
    }
    section = section.set("READY", "1").set("ERRORLEVEL", "0");
    return ini;
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
