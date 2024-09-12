use std::convert::From;
use std::fs::File;
use std::path::PathBuf;
use xml::attribute::OwnedAttribute;
use xml::name::OwnedName;
use xml::reader::EventReader;
use xml::reader::XmlEvent;
use xml::writer::EventWriter;

use crate::gdt::{GdtFile, GdtPatientGender};

#[derive(Debug)]
pub enum OppError {
    IoError(std::io::Error),
    XmlReaderError(xml::reader::Error),
    XmlWriterError(xml::writer::Error),
}

impl From<std::io::Error> for OppError {
    fn from(error: std::io::Error) -> Self {
        OppError::IoError(error)
    }
}

impl From<xml::reader::Error> for OppError {
    fn from(error: xml::reader::Error) -> Self {
        OppError::XmlReaderError(error)
    }
}

impl From<xml::writer::Error> for OppError {
    fn from(error: xml::writer::Error) -> Self {
        OppError::XmlWriterError(error)
    }
}

pub fn default_xml_str() -> String {
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<opp version="1.2">
    <command type="UPDATE_CREATE" silent="NO"/>
    <software name="gdt2dicom" version="0.1.0"/>
</opp>
    "#
    );
    return xml;
}

pub fn insert_patient_to_xml(file: GdtFile, events: &mut Vec<XmlEvent>) {
    let end_index = events
        .iter()
        .position(|e| match e {
            XmlEvent::EndElement {
                name: OwnedName { local_name, .. },
            } if local_name.as_str() == "opp" => true,
            _ => false,
        })
        .unwrap_or(events.len() - 2);
    let start = XmlEvent::StartElement {
        name: OwnedName {
            local_name: "patient".to_string(),
            namespace: None,
            prefix: None,
        },
        attributes: vec![
            OwnedAttribute {
                name: OwnedName {
                    local_name: "reference".to_string(),
                    namespace: None,
                    prefix: None,
                },
                value: file.object_patient.patient_number,
            },
            OwnedAttribute {
                name: OwnedName {
                    local_name: "firstName".to_string(),
                    namespace: None,
                    prefix: None,
                },
                value: file.object_patient.patient_first_name,
            },
            OwnedAttribute {
                name: OwnedName {
                    local_name: "lastName".to_string(),
                    namespace: None,
                    prefix: None,
                },
                value: file.object_patient.patient_name,
            },
            OwnedAttribute {
                name: OwnedName {
                    local_name: "birthdate".to_string(),
                    namespace: None,
                    prefix: None,
                },
                value: gdt_date_to_opp(file.object_patient.patient_dob),
            },
            OwnedAttribute {
                name: OwnedName {
                    local_name: "gender".to_string(),
                    namespace: None,
                    prefix: None,
                },
                value: match file.object_patient.patient_gender {
                    GdtPatientGender::Male => "MALE".to_string(),
                    GdtPatientGender::Female => "FEMALE".to_string(),
                },
            },
        ],
        namespace: xml::namespace::Namespace::empty(),
    };

    let end = XmlEvent::EndElement {
        name: OwnedName {
            local_name: "patient".to_string(),
            namespace: None,
            prefix: None,
        },
    };
    events.insert(end_index, start);
    events.insert(end_index + 1, end);
}

pub fn file_to_xml(file: GdtFile, output: PathBuf) -> Result<File, OppError> {
    let xml_str = default_xml_str();
    let reader = EventReader::new(xml_str.as_bytes());
    let mut events: Vec<XmlEvent> = reader
        .into_iter()
        .filter(|e| match e {
            Ok(XmlEvent::StartDocument { .. }) | Ok(XmlEvent::EndDocument) => false,
            _ => true,
        })
        .collect::<Result<Vec<_>, xml::reader::Error>>()
        .unwrap();
    insert_patient_to_xml(file, &mut events);
    let file = File::create(output)?;
    let mut writer = EventWriter::new(&file);
    for e in events {
        match e.as_writer_event() {
            Some(e) => writer.write(e)?,
            _ => (), // events like EndDocument are ignored
        };
    }
    return Ok(file);
}

fn gdt_date_to_opp(str: String) -> String {
    // DDMMYYYY -> YYYY-MM-DD
    let day = &str[0..2];
    let month = &str[2..4];
    let year = &str[4..8];
    return format!("{}-{}-{}", year, month, day);
}
