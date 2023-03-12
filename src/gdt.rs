use std::fs::File;
use std::io::BufReader;
use std::io::BufRead;
use log::{error};
use std::path::Path;
use std::str::FromStr;
use std::result::Result;

#[derive(Debug, Default)]
pub struct GdtFile {
    record_type: u32,
    record_length: u32,
    objects: Vec<GdtObject>,
}

#[derive(Debug, Default)]
pub struct GdtObject {
    object_type: String,
    gdt_id_receiver: String, // 8315, GDT-ID of the receiver
    gdt_id_sender: String, // 8316, GDT-ID of the sender
    version_gdt: String, // 9218, Version GDT
    patient_number: String, // 3000
    patient_name: String, // 3101
    patient_first_name: String, // 3102
    patient_dob: String, // 3103
    patient_gender: GdtPatientGender, // 3110
    patient_height: String, // 3622, cm
    patient_weight: String, // 3623 / 3632? kg
}

// object types:
// Obj_Anforderung (Obj_request)
// Obj_Anhang (Obj_annex)
// Obj_Arztidentifikation (Obj_physician_identification)
// Obj_Basisdiagnostikdia (Obj_basic_diagnostics
// Obj_Dauerdiagnosis (Obj_permanent_diagnosis)
// Obj_Dauermedikament (Obj_permanent_medication)
// Obj_Diagnosis
// Obj_Einweisung (Obj_admission)
// Obj_Kopfdaten (Obj_header_data)
// Obj_Patient
// Obj_RgEmpfänger (Obj_invoice_recipient)
// Obj_Satzende (Obj_end_of_record)
// Obj_Schein (Obj_certificate)
// Obj_Terminanfrage (Obj_appointment_request)
// Obj_Ueberweisung (Obj_referral)
// Obj_Versichertenkarte (Obj_health-insurance_card)

#[derive(Debug)]
pub enum GdtPatientGender {
    Male,
    Female,
}

impl Default for GdtPatientGender {
    fn default() -> GdtPatientGender {
        GdtPatientGender::Male
    }
}

#[derive(Debug)]
pub struct RawGdtLine {
    field_identifier: u32,
    content: String,
}

#[derive(Debug)]
pub enum GdtError {
    IoError(std::io::Error),
    FieldIdentifierNotNumber(String, std::num::ParseIntError),
    LineTooShort(String),
    LineNotFound(String),
    NumberExpected(String, std::num::ParseIntError),
    InvalidValue(String, String),
}

pub fn parse_file_lines<P>(path: P) -> Result<impl std::iter::Iterator<Item = Result<RawGdtLine, GdtError>>, GdtError> where P: AsRef<Path> {
    let file = File::open(path).map_err(GdtError::IoError)?;
    let reader = BufReader::new(file);

    return Ok(reader.lines().map(|r_str|
        r_str.map_err(GdtError::IoError).and_then(string_to_gdt_line)
    ))
}

fn string_to_gdt_line(str: String) -> Result<RawGdtLine, GdtError> {
    if str.len() < 7 {
        return Err(GdtError::LineTooShort(str));
    }
    let field_id = u32::from_str(&str[3..7]).map_err(|e| GdtError::FieldIdentifierNotNumber(String::from(&str), e))?;

    return Ok(RawGdtLine {
        field_identifier: field_id,
        content: String::from(&str[7..]),
    });
}

pub fn parse_file<P>(path: P) -> Result<GdtFile, GdtError> where P: AsRef<Path> {
    let mut file = Default::default();
    let mut iter = parse_file_lines(path)?;
    read_record_header(&mut file, &mut iter)?;
    while let Some(r_next_line) = iter.next() {
        match r_next_line {
            Err(e) => error!("Error in line: {:?}", e),
            Ok(line@RawGdtLine { field_identifier: 8200, .. }) => {
                let obj = read_record_object(line, &mut iter)?;
                file.objects.push(obj);
            },
            Ok(RawGdtLine { field_identifier: 8202, .. }) => {
                return Ok(file);
            },
            _ => {}
        }
    }
    return Ok(file);
}

type GdtLineIter<'a> = dyn 'a + std::iter::Iterator<Item = Result<RawGdtLine, GdtError>>;

fn read_record_header(file: &mut GdtFile, iter: &mut GdtLineIter) -> Result<(), GdtError> {
    let first_line = iter.next().ok_or_else(|| GdtError::LineNotFound("0".to_string())).and_then(|x| x)?;
    let first_line_content = u32::from_str(&first_line.content).map_err(|e| GdtError::NumberExpected(first_line.content, e))?;
    
    let second_line = iter.next().ok_or_else(|| GdtError::LineNotFound("1".to_string())).and_then(|x| x)?;
    let second_line_content = u32::from_str(&second_line.content).map_err(|e| GdtError::NumberExpected(second_line.content, e))?;

    file.record_type = first_line_content;
    file.record_length = second_line_content;
    return Ok(())
}

fn read_record_object(line: RawGdtLine, iter: &mut GdtLineIter) -> Result<GdtObject, GdtError> {
    let mut new_obj = GdtObject { object_type: line.content, ..Default::default() };
    while let Some(r_next_line) = iter.next() {
        match r_next_line {
            Err(e) => error!("Error in line: {:?}", e),
            Ok(RawGdtLine { field_identifier: 8315, content }) => {
                new_obj.gdt_id_receiver = content;
            },
            Ok(RawGdtLine { field_identifier: 8316, content }) => {
                new_obj.gdt_id_sender = content;
            }
            Ok(RawGdtLine { field_identifier: 9218, content }) => {
                new_obj.version_gdt = content;
            }
            Ok(RawGdtLine { field_identifier: 3000, content }) => {
                new_obj.patient_number = content;
            },
            Ok(RawGdtLine { field_identifier: 3101, content }) => {
                new_obj.patient_name = content;
            },
            Ok(RawGdtLine { field_identifier: 3102, content }) => {
                new_obj.patient_first_name = content;
            },
            Ok(RawGdtLine { field_identifier: 3103, content }) => {
                new_obj.patient_dob = content;
            },
            Ok(RawGdtLine { field_identifier: 3110, content }) => {
                if content == "1" {
                    new_obj.patient_gender = GdtPatientGender::Male;
                } else if content == "2" {
                    new_obj.patient_gender = GdtPatientGender::Female;
                } else {
                    return Err(GdtError::InvalidValue("Gender".to_string(), content));
                }
            },
            Ok(RawGdtLine { field_identifier: 3622, content }) => {
                new_obj.patient_height = content;
            },
            Ok(RawGdtLine { field_identifier: 3623, content }) => {
                new_obj.patient_weight = content;
            },
            Ok(RawGdtLine { field_identifier: 3632, content }) => {
                new_obj.patient_weight = content;
            },
            Ok(RawGdtLine { field_identifier: 8201, .. }) => {
                return Ok(new_obj);
            }
            _ => {}
        }
    }
    return Ok(new_obj);
}
