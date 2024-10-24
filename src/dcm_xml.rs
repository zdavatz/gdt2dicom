use std::ffi::OsStr;
use std::fs::File;
use std::io::{Error, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;
use std::sync::mpsc;
use std::time::SystemTime;

use chrono::{DateTime, Utc};
use regex::Regex;
use tempfile::NamedTempFile;
use xml::attribute::OwnedAttribute;
use xml::name::OwnedName;
use xml::reader::EventReader;
use xml::reader::XmlEvent;
use xml::writer::EventWriter;

use crate::command::{exec_command, ChildOutput};
use crate::error::G2DError;
use crate::gdt::{GdtBasicDiagnosticsObject, GdtFile, GdtPatientGender, GdtPatientObject};

pub fn parse_dcm_xml(path: &Path) -> Result<Vec<XmlEvent>, G2DError> {
    let file = File::open(path)?;
    let reader = EventReader::new(file);
    let mut events: Vec<XmlEvent> = reader
        .into_iter()
        .collect::<Result<Vec<_>, xml::reader::Error>>()?;
    add_meta_header_if_not_exist(DcmTransferType::JPEGBaseline, &mut events);
    return Ok(events);
}

pub fn parse_dcm_as_xml(path: &PathBuf) -> Result<Vec<XmlEvent>, G2DError> {
    let output = Command::new("dcm2xml").arg(path).output()?;
    std::io::stderr().write_all(&output.stderr).unwrap();
    let reader = EventReader::new(output.stdout.as_slice());
    let events: Vec<XmlEvent> = reader
        .into_iter()
        .collect::<Result<Vec<_>, xml::reader::Error>>()?;
    return Ok(events);
}

#[derive(Debug)]
pub enum DCMImageFormat {
    Jpeg,
    Png,
}

pub fn export_images_from_dcm_with_patient_id(
    dcm_path: &PathBuf,
    output_path: &PathBuf,
    format: DCMImageFormat,
    log_sender: Option<&mpsc::Sender<ChildOutput>>,
) -> Result<Vec<String>, G2DError> {
    let dcm_xml_events = parse_dcm_as_xml(&dcm_path)?;
    let patient_id = xml_get_patient_patient_id(&dcm_xml_events);
    let patient_id = match patient_id {
        Some(id) => id,
        None => {
            if let Some(l) = log_sender {
                _ = l.send(ChildOutput::Log(
                    "Cannot patient id from Dicom file".to_string(),
                ));
            }
            let custom_error =
                Error::new(ErrorKind::Other, "Cannot find patient id from Dicom file");
            return Err(G2DError::IoError(custom_error));
        }
    };
    let mut output_path = output_path.clone();
    output_path.push("_gdt2dicom_temp_");
    let saved_files = export_images_from_dcm(dcm_path, &output_path, format, log_sender)?;
    for saved_file in &saved_files {
        let new_name = saved_file.replace("_gdt2dicom_temp_.", &format!("{}_", patient_id));
        std::fs::rename(saved_file, new_name)?;
    }
    return Ok(saved_files);
}

pub fn export_images_from_dcm(
    dcm_path: &PathBuf,
    output_path: &PathBuf,
    format: DCMImageFormat,
    log_sender: Option<&mpsc::Sender<ChildOutput>>,
) -> Result<Vec<String>, G2DError> {
    let output = exec_command(
        "dcmj2pnm",
        vec![
            match format {
                DCMImageFormat::Png => OsStr::new("--write-png"),
                DCMImageFormat::Jpeg => OsStr::new("--write-jpeg"),
            },
            dcm_path.as_os_str(),
            output_path.as_os_str(),
            OsStr::new("--all-frames"),
            OsStr::new("--verbose"),
        ],
        false,
        None,
    )?;
    let err_str = std::str::from_utf8(&output.stderr).unwrap();
    let mut output_filenames = vec![];
    if output.status.success() {
        let re = Regex::new(r"I: writing frame [0-9]+ to (.*)").unwrap();
        for (_, [x]) in re.captures_iter(err_str).map(|c| c.extract()) {
            output_filenames.push(x.to_string());
        }
    } else {
        if let Some(l) = log_sender {
            _ = l.send(ChildOutput::Log(format!("Error: {:?}", err_str)));
        }
    }
    return Ok(output_filenames);
}

pub fn xml_events_to_file(events: Vec<XmlEvent>) -> Result<NamedTempFile, G2DError> {
    let temp_file = NamedTempFile::new()?;
    let mut writer = EventWriter::new(&temp_file);
    for e in events {
        match e.as_writer_event() {
            Some(e) => writer.write(e)?,
            _ => (), // events like EndDocument are ignored
        };
    }
    return Ok(temp_file);
}

fn xml_contains(xml_events: &Vec<XmlEvent>, tag: String, name: String) -> bool {
    xml_events.iter().any(|ev| match ev {
        XmlEvent::StartElement {
            name: xml::name::OwnedName { local_name, .. },
            attributes,
            ..
        } if local_name.as_str() == "element"
            && attributes_contain(attributes, tag.clone(), name.clone()) =>
        {
            true
        }
        _ => false,
    })
}

fn value_of_attribute(attrs: &Vec<OwnedAttribute>, name: &str) -> Option<String> {
    attrs.iter().find_map(
        |OwnedAttribute {
             name: xml::name::OwnedName { local_name, .. },
             value,
         }| {
            if *local_name == name {
                Some(value.clone())
            } else {
                None
            }
        },
    )
}

pub struct DcmElement {
    pub tag: String,
    pub vr: String,
    pub name: String,
    pub body: String,
}

fn add_element_if_not_exist(events: &mut Vec<XmlEvent>, element: DcmElement) {
    if xml_contains(events, element.tag.clone(), element.name.clone()) {
        return;
    }
    let end_data_set_index = events
        .iter()
        .position(|e| match e {
            XmlEvent::EndElement {
                name: OwnedName { local_name, .. },
            } if local_name.as_str() == "data-set" => true,
            _ => false,
        })
        .unwrap_or(events.len() - 2);
    let start = XmlEvent::StartElement {
        name: OwnedName {
            local_name: "element".to_string(),
            namespace: None,
            prefix: None,
        },
        attributes: vec![
            OwnedAttribute {
                name: OwnedName {
                    local_name: "tag".to_string(),
                    namespace: None,
                    prefix: None,
                },
                value: element.tag,
            },
            OwnedAttribute {
                name: OwnedName {
                    local_name: "vr".to_string(),
                    namespace: None,
                    prefix: None,
                },
                value: element.vr,
            },
            OwnedAttribute {
                name: OwnedName {
                    local_name: "vm".to_string(),
                    namespace: None,
                    prefix: None,
                },
                value: "1".to_string(),
            },
            OwnedAttribute {
                name: OwnedName {
                    local_name: "len".to_string(),
                    namespace: None,
                    prefix: None,
                },
                value: "0".to_string(), // TODO
            },
            OwnedAttribute {
                name: OwnedName {
                    local_name: "name".to_string(),
                    namespace: None,
                    prefix: None,
                },
                value: element.name,
            },
        ],
        namespace: xml::namespace::Namespace::empty(),
    };

    let body = XmlEvent::Characters(element.body);
    let end = XmlEvent::EndElement {
        name: OwnedName {
            local_name: "element".to_string(),
            namespace: None,
            prefix: None,
        },
    };
    events.insert(end_data_set_index, start);
    events.insert(end_data_set_index + 1, body);
    events.insert(end_data_set_index + 2, end);
}

fn attributes_contain(attrs: &Vec<OwnedAttribute>, tag: String, name: String) -> bool {
    let xml_tag = value_of_attribute(attrs, "tag");
    let xml_name = value_of_attribute(attrs, "name");
    return xml_tag == Some(tag) && xml_name == Some(name);
}

fn add_meta_header_if_not_exist(transfer_type: DcmTransferType, events: &mut Vec<XmlEvent>) {
    let has_meta_header = events.iter().any(|e| match e {
        XmlEvent::StartElement {
            name: OwnedName { local_name, .. },
            ..
        } if local_name.as_str() == "meta-header" => true,
        _ => false,
    });
    if has_meta_header {
        return;
    }
    let open_data_set_index = events
        .iter()
        .position(|e| match e {
            XmlEvent::StartElement {
                name: OwnedName { local_name, .. },
                ..
            } if local_name.as_str() == "data-set" => true,
            _ => false,
        })
        .unwrap_or(2);
    let extra_elements = match transfer_type {
        DcmTransferType::JPEGBaseline => {
            r#"<element tag="0002,0002" vr="UI" vm="1" len="28" name="MediaStorageSOPClassUID">1.2.840.10008.5.1.4.1.1.7.2</element>"#
        }
        DcmTransferType::LittleEndianExplicit => "",
    };
    let xml = format!(
        r#"
    <meta-header xfer="1.2.840.10008.1.2.1" name="Little Endian Explicit">
    {extra_elements}
    <element tag="0002,0000" vr="UL" vm="1" len="4" name="FileMetaInformationGroupLength">0</element>
    <element tag="0002,0003" vr="UI" vm="1" len="50" name="MediaStorageSOPInstanceUID"></element>
    <element tag="0002,0010" vr="UI" vm="1" len="22" name="TransferSyntaxUID"></element>
    <element tag="0002,0012" vr="UI" vm="1" len="28" name="ImplementationClassUID"></element>
    <element tag="0002,0013" vr="SH" vm="1" len="16" name="ImplementationVersionName"></element>
    </meta-header>"#,
        extra_elements = extra_elements
    );
    let reader = EventReader::new(xml.as_bytes());
    let new_events: Vec<XmlEvent> = reader
        .into_iter()
        .filter(|e| match e {
            Ok(XmlEvent::StartDocument { .. }) | Ok(XmlEvent::EndDocument) => false,
            _ => true,
        })
        .collect::<Result<Vec<_>, xml::reader::Error>>()
        .unwrap();
    events.splice(
        (open_data_set_index - 1)..(open_data_set_index - 1),
        new_events,
    );
}

#[derive(Debug)]
pub enum DcmTransferType {
    JPEGBaseline,
    LittleEndianExplicit,
}

pub fn default_dcm_xml(transfer_type: DcmTransferType) -> Vec<XmlEvent> {
    let xml = match transfer_type {
        DcmTransferType::JPEGBaseline => default_dcm_xml_str(),
        DcmTransferType::LittleEndianExplicit => default_dcm_worklist_xml_str(),
    };
    let reader = EventReader::new(xml.as_bytes());
    let mut events: Vec<XmlEvent> = reader
        .into_iter()
        .filter(|e| match e {
            Ok(XmlEvent::StartDocument { .. }) | Ok(XmlEvent::EndDocument) => false,
            _ => true,
        })
        .collect::<Result<Vec<_>, xml::reader::Error>>()
        .unwrap();
    add_meta_header_if_not_exist(transfer_type, &mut events);
    return events;
}

pub fn default_dcm_xml_str() -> String {
    let curr_time = SystemTime::now();
    let dt: DateTime<Utc> = curr_time.clone().into();
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<file-format>
<data-set xfer="1.2.840.10008.1.2.4.50" name="JPEG Baseline">
<element tag="0008,0016" vr="UI" vm="1" len="28" name="SOPClassUID">1.2.840.10008.5.1.4.1.1.7.2</element>
<element tag="0008,0018" vr="UI" vm="1" len="50" name="SOPInstanceUID"></element>
<element tag="0008,0020" vr="DA" vm="0" len="0" name="StudyDate"></element>
<element tag="0008,0030" vr="TM" vm="0" len="0" name="StudyTime"></element>
<element tag="0008,0050" vr="SH" vm="0" len="0" name="AccessionNumber"></element>
<element tag="0008,0090" vr="PN" vm="0" len="0" name="ReferringPhysicianName"></element>
<element tag="0020,0010" vr="SH" vm="0" len="0" name="StudyID"></element>
<element tag="0020,0011" vr="IS" vm="0" len="0" name="SeriesNumber"></element>
<element tag="0020,0013" vr="IS" vm="0" len="0" name="InstanceNumber"></element>
<sequence tag="0040,0100" vr="SQ" card="1" len="24" name="ScheduledProcedureStepSequence">
<item card="1" len="16">
<element tag="0040,0002" vr="DA" vm="1" len="8" name="ScheduledProcedureStepStartDate">{today}</element>
<element tag="0040,0003" vr="TM" vm="1" len="6" name="ScheduledProcedureStepStartTime">{current_time}</element>
</item>
</sequence>
</data-set>
</file-format>
    "#,
        today = dt.format("%Y%m%d"),
        current_time = dt.format("%H%M%S")
    );
    return xml;
}

pub fn default_dcm_worklist_xml_str() -> String {
    let curr_time = SystemTime::now();
    let dt: DateTime<Utc> = curr_time.clone().into();
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<file-format>
<data-set xfer="1.2.840.10008.1.2.1" name="Little Endian Explicit">
<element tag="0008,0020" vr="DA" vm="0" len="0" name="StudyDate"></element>
<element tag="0008,0030" vr="TM" vm="0" len="0" name="StudyTime"></element>
<element tag="0008,0050" vr="SH" vm="0" len="0" name="AccessionNumber"></element>
<element tag="0008,0090" vr="PN" vm="0" len="0" name="ReferringPhysicianName"></element>
<element tag="0020,000d" vr="UI" vm="1" len="26" name="StudyInstanceUID">1.2.276.0.7230010.3.2.109</element>
<element tag="0020,0010" vr="SH" vm="0" len="0" name="StudyID"></element>
<element tag="0020,0011" vr="IS" vm="0" len="0" name="SeriesNumber"></element>
<element tag="0020,0013" vr="IS" vm="0" len="0" name="InstanceNumber"></element>
<sequence tag="0040,0100" vr="SQ" card="1" len="24" name="ScheduledProcedureStepSequence">
<item card="1" len="16">
<element tag="0040,0002" vr="DA" vm="1" len="8" name="ScheduledProcedureStepStartDate">{today}</element>
<element tag="0040,0003" vr="TM" vm="1" len="6" name="ScheduledProcedureStepStartTime">{current_time}</element>
</item>
</sequence>
</data-set>
</file-format>
    "#,
        today = dt.format("%Y%m%d"),
        current_time = dt.format("%H%M%S")
    );
    return xml;
}

pub fn file_to_xml(file: GdtFile, xml_events: &Vec<XmlEvent>) -> Result<NamedTempFile, G2DError> {
    let mut cloned = xml_events.clone();
    add_element_if_not_exist(
        &mut cloned,
        DcmElement {
            tag: "0010,0010".to_string(),
            vr: "PN".to_string(),
            name: "PatientName".to_string(),
            body: gdt_get_patient_name(&file.object_patient),
        },
    );

    add_element_if_not_exist(
        &mut cloned,
        DcmElement {
            tag: "0010,0020".to_string(),
            vr: "LO".to_string(),
            name: "PatientID".to_string(),
            body: file.object_patient.patient_number,
        },
    );
    add_element_if_not_exist(
        &mut cloned,
        DcmElement {
            tag: "0010,0030".to_string(),
            vr: "DA".to_string(),
            name: "PatientBirthDate".to_string(),
            body: gdt_date_to_dcm(file.object_patient.patient_dob),
        },
    );
    add_element_if_not_exist(
        &mut cloned,
        DcmElement {
            tag: "0010,0040".to_string(),
            vr: "CS".to_string(),
            name: "PatientSex".to_string(),
            body: gdt_gender_to_dcm(&file.object_patient.patient_gender),
        },
    );
    add_element_if_not_exist(
        &mut cloned,
        DcmElement {
            tag: "0010,1020".to_string(),
            vr: "DS".to_string(),
            name: "PatientSize".to_string(),
            body: gdt_get_patient_height_in_meters(&file.object_basic_diagnostics)
                .unwrap_or("".to_string()),
        },
    );
    add_element_if_not_exist(
        &mut cloned,
        DcmElement {
            tag: "0010,1030".to_string(),
            vr: "DS".to_string(),
            name: "PatientWeight".to_string(),
            body: file.object_basic_diagnostics.patient_weight,
        },
    );
    return xml_events_to_file(cloned);
}

fn gdt_date_to_dcm(str: String) -> String {
    // DDMMYYYY -> YYYYMMDD
    let day = &str[0..2];
    let month = &str[2..4];
    let year = &str[4..8];
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
        return format!(
            "{}^{}",
            patient.patient_name.to_uppercase(),
            patient.patient_first_name.to_uppercase(),
        )
        .replace(" ", "^");
    } else if patient.patient_name.len() > 0 {
        return format!("{}", patient.patient_name.to_uppercase());
    } else {
        return format!("{}", patient.patient_first_name.to_uppercase());
    }
}

fn gdt_get_patient_height_in_meters(patient: &GdtBasicDiagnosticsObject) -> Option<String> {
    let num = f64::from_str(&patient.patient_height).ok()?;
    return Some(format!("{}", num / 100.0));
}

fn xml_get_element_body(
    events: &Vec<XmlEvent>,
    in_name: Option<String>,
    in_tag: Option<String>,
) -> Option<String> {
    let start_tag_index = events.iter().position(|e| match e {
        XmlEvent::StartElement {
            name: OwnedName { local_name, .. },
            attributes,
            ..
        } if local_name.as_str() == "element"
            && value_of_attribute(&attributes, "name") == in_name
            && value_of_attribute(&attributes, "tag") == in_tag =>
        {
            true
        }
        _ => false,
    })?;
    let XmlEvent::Characters(result) = &events[start_tag_index + 1] else {
        return None;
    };
    return Some(result.clone());
}

pub fn xml_get_patient_name(events: &Vec<XmlEvent>) -> Option<String> {
    return xml_get_element_body(
        &events,
        Some("PatientName".to_string()),
        Some("0010,0010".to_string()),
    );
}

pub fn xml_get_patient_height_meter(events: &Vec<XmlEvent>) -> Option<String> {
    return xml_get_element_body(
        &events,
        Some("PatientSize".to_string()),
        Some("0010,1020".to_string()),
    );
}

pub fn xml_get_patient_weight_kg(events: &Vec<XmlEvent>) -> Option<String> {
    return xml_get_element_body(
        &events,
        Some("PatientWeight".to_string()),
        Some("0010,1030".to_string()),
    );
}

pub fn xml_get_patient_patient_id(events: &Vec<XmlEvent>) -> Option<String> {
    return xml_get_element_body(
        &events,
        Some("PatientID".to_string()),
        Some("0010,0020".to_string()),
    );
}

pub fn xml_get_patient_birth_date(events: &Vec<XmlEvent>) -> Option<String> {
    return xml_get_element_body(
        &events,
        Some("PatientBirthDate".to_string()),
        Some("0010,0030".to_string()),
    );
}

pub fn xml_get_patient_sex(events: &Vec<XmlEvent>) -> Option<String> {
    return xml_get_element_body(
        &events,
        Some("PatientSex".to_string()),
        Some("0010,0040".to_string()),
    );
}

pub fn xml_get_study_date(events: &Vec<XmlEvent>) -> Option<String> {
    return xml_get_element_body(
        &events,
        Some("StudyDate".to_string()),
        Some("0008,0020".to_string()),
    );
}

pub fn xml_get_study_time(events: &Vec<XmlEvent>) -> Option<String> {
    return xml_get_element_body(
        &events,
        Some("StudyTime".to_string()),
        Some("0008,0030".to_string()),
    );
}
