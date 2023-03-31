use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;

use tempfile::NamedTempFile;
use xml::attribute::OwnedAttribute;
use xml::name::OwnedName;
use xml::reader::EventReader;
use xml::reader::XmlEvent;
use xml::writer::EventWriter;

use crate::gdt::{GdtBasicDiagnosticsObject, GdtFile, GdtPatientGender, GdtPatientObject};

#[derive(Debug)]
pub enum DcmError {
    IoError(std::io::Error),
    XmlReaderError(xml::reader::Error),
    XmlWriterError(xml::writer::Error),
}

pub fn parse_dcm_xml(path: &Path) -> Result<Vec<XmlEvent>, DcmError> {
    let file = File::open(path).map_err(DcmError::IoError)?;
    let reader = EventReader::new(file);
    let mut events: Vec<XmlEvent> = reader
        .into_iter()
        .collect::<Result<Vec<_>, xml::reader::Error>>()
        .map_err(DcmError::XmlReaderError)?;
    add_meta_header_if_not_exist(&mut events);
    return Ok(events);
}

pub fn parse_dcm_as_xml(path: &PathBuf) -> Result<Vec<XmlEvent>, DcmError> {
    let output = Command::new("dcm2xml")
        .arg(path)
        .output()
        .map_err(DcmError::IoError)?;
    std::io::stderr().write_all(&output.stderr).unwrap();
    let reader = EventReader::new(output.stdout.as_slice());
    let events: Vec<XmlEvent> = reader
        .into_iter()
        .collect::<Result<Vec<_>, xml::reader::Error>>()
        .map_err(DcmError::XmlReaderError)?;
    return Ok(events);
}

pub fn export_images_from_dcm(dcm_path: &PathBuf, output_path: &PathBuf) -> Result<(), DcmError> {
    // dcmj2pnm --write-png /Users/b123400/Downloads/0002.DCM  ./output --all-frames
    let output = Command::new("dcmj2pnm")
        .arg("--write-png")
        .arg(dcm_path)
        .arg(output_path)
        .arg("--all-frames")
        .output()
        .map_err(DcmError::IoError)?;
    std::io::stderr().write_all(&output.stderr).unwrap();
    std::io::stdout().write_all(&output.stdout).unwrap();
    return Ok(());
}

pub fn xml_events_to_file(events: Vec<XmlEvent>) -> Result<NamedTempFile, DcmError> {
    let temp_file = NamedTempFile::new().map_err(DcmError::IoError)?;
    // std::fs::write(&temp_file, xml)?;
    let mut writer = EventWriter::new(&temp_file);
    for e in events {
        match e.as_writer_event() {
            Some(e) => writer.write(e).map_err(DcmError::XmlWriterError)?,
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

fn value_of_attribute(attrs: &Vec<OwnedAttribute>, name: String) -> Option<String> {
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
    let xml_tag = value_of_attribute(attrs, "tag".to_string());
    let xml_name = value_of_attribute(attrs, "name".to_string());
    return xml_tag == Some(tag) && xml_name == Some(name);
}

fn add_meta_header_if_not_exist(events: &mut Vec<XmlEvent>) {
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
    let xml = r#"
    <meta-header xfer="1.2.840.10008.1.2.1" name="Little Endian Explicit">
    <element tag="0002,0000" vr="UL" vm="1" len="4" name="FileMetaInformationGroupLength">0</element>
    <element tag="0002,0002" vr="UI" vm="1" len="28" name="MediaStorageSOPClassUID">1.2.840.10008.5.1.4.1.1.7.2</element>
    <element tag="0002,0003" vr="UI" vm="1" len="50" name="MediaStorageSOPInstanceUID"></element>
    <element tag="0002,0010" vr="UI" vm="1" len="22" name="TransferSyntaxUID"></element>
    <element tag="0002,0012" vr="UI" vm="1" len="28" name="ImplementationClassUID"></element>
    <element tag="0002,0013" vr="SH" vm="1" len="16" name="ImplementationVersionName"></element>
    </meta-header>"#;
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

pub fn default_dcm_xml() -> Vec<XmlEvent> {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
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
</data-set>
</file-format>
    "#;
    let reader = EventReader::new(xml.as_bytes());
    let mut events: Vec<XmlEvent> = reader
        .into_iter()
        .filter(|e| match e {
            Ok(XmlEvent::StartDocument { .. }) | Ok(XmlEvent::EndDocument) => false,
            _ => true,
        })
        .collect::<Result<Vec<_>, xml::reader::Error>>()
        .unwrap();
    add_meta_header_if_not_exist(&mut events);
    return events;
}

pub fn file_to_xml(file: GdtFile, xml_events: &Vec<XmlEvent>) -> Result<NamedTempFile, DcmError> {
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
