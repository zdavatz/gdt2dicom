use crate::gdt::{ GdtBasicDiagnosticsObject, GdtFile, GdtPatientGender, GdtPatientObject };
use std::str::FromStr;

pub fn file_to_xml(file: GdtFile) -> String {
    return format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<file-format>
<meta-header xfer="1.2.840.10008.1.2.1" name="Little Endian Explicit">
<element tag="0002,0000" vr="UL" vm="1" len="4" name="FileMetaInformationGroupLength">0</element>
<element tag="0002,0002" vr="UI" vm="1" len="28" name="MediaStorageSOPClassUID">1.2.840.10008.5.1.4.1.1.7.2</element>
<element tag="0002,0003" vr="UI" vm="1" len="50" name="MediaStorageSOPInstanceUID"></element>
<element tag="0002,0010" vr="UI" vm="1" len="22" name="TransferSyntaxUID"></element>
<element tag="0002,0012" vr="UI" vm="1" len="28" name="ImplementationClassUID"></element>
<element tag="0002,0013" vr="SH" vm="1" len="16" name="ImplementationVersionName"></element>
</meta-header>
<data-set xfer="1.2.840.10008.1.2.4.50" name="JPEG Baseline">
<element tag="0008,0016" vr="UI" vm="1" len="28" name="SOPClassUID">1.2.840.10008.5.1.4.1.1.7.2</element>
<element tag="0008,0018" vr="UI" vm="1" len="50" name="SOPInstanceUID"></element>
<element tag="0008,0020" vr="DA" vm="0" len="0" name="StudyDate"></element>
<element tag="0008,0030" vr="TM" vm="0" len="0" name="StudyTime"></element>
<element tag="0008,0050" vr="SH" vm="0" len="0" name="AccessionNumber"></element>
<element tag="0008,0090" vr="PN" vm="0" len="0" name="ReferringPhysicianName"></element>
<element tag="0010,0010" vr="PN" vm="0" len="0" name="PatientName">{patient_name}</element>
<element tag="0010,0020" vr="LO" vm="0" len="0" name="PatientID">{patient_id}</element>
<element tag="0010,0030" vr="DA" vm="0" len="0" name="PatientBirthDate">{patient_dob}</element>
<element tag="0010,0040" vr="CS" vm="0" len="0" name="PatientSex">{patient_sex}</element>
<element tag="0010,1020" vr="DS" vm="0" len="0" name="PatientSize">{patient_size}</element>
<element tag="0010,1030" vr="DS" vm="0" len="0" name="PatientWeight">{patient_weight}</element>
<element tag="0020,0010" vr="SH" vm="0" len="0" name="StudyID"></element>
<element tag="0020,0011" vr="IS" vm="0" len="0" name="SeriesNumber"></element>
<element tag="0020,0013" vr="IS" vm="0" len="0" name="InstanceNumber"></element>
</data-set>
</file-format>
    "#,
    patient_name=gdt_get_patient_name(&file.object_patient),
    patient_id=file.object_patient.patient_number,
    patient_dob=gdt_date_to_dcm(file.object_patient.patient_dob),
    patient_sex=gdt_gender_to_dcm(&file.object_patient.patient_gender),
    patient_size=gdt_get_patient_height_in_meters(&file.object_basic_diagnostics).unwrap_or("".to_string()),
    patient_weight=file.object_basic_diagnostics.patient_weight,
    )
}

fn gdt_date_to_dcm(str: String) -> String {
    // DDMMYYYY -> YYYYMMDD
    let day = &str[0..2];
    let year = &str[4..8];
    let month = &str[2..4];
    return format!("{}{}{}", year, month, day);
}

fn gdt_gender_to_dcm(gender: &GdtPatientGender) -> String {
    match gender {
        crate::gdt::GdtPatientGender::Male => "M".to_string(),
        crate::gdt::GdtPatientGender::Female => "F".to_string(),
    }
}

fn gdt_get_patient_name(patient: &GdtPatientObject) -> String {
    if patient.patient_name.len() > 0 && patient.patient_first_name.len() > 0 {
        return format!("{} {}", patient.patient_first_name, patient.patient_name);
    } else if patient.patient_name.len() > 0 {
        return format!("{}", patient.patient_name);
    } else {
        return format!("{}", patient.patient_first_name);
    }
}

fn gdt_get_patient_height_in_meters(patient: &GdtBasicDiagnosticsObject) -> Option<String> {
    let num = f64::from_str(&patient.patient_height).ok()?;
    return Some(format!("{}", num / 100.0));
}
