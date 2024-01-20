use log::error;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::path::Path;
use std::result::Result;
use std::str::FromStr;
use xml::reader::XmlEvent;

use crate::dcm_xml::{
    xml_get_patient_birth_date, xml_get_patient_height_meter, xml_get_patient_name,
    xml_get_patient_patient_id, xml_get_patient_sex, xml_get_patient_weight_kg, xml_get_study_date,
    xml_get_study_time,
};

#[derive(Debug, Default)]
pub struct GdtFile {
    pub record_type: u32,
    pub record_length: u32,
    pub object_request: GdtRequestObject, // Obj_Anforderung
    pub object_annex: GdtAnnexObject,     // Obj_Anhang
    pub object_physician_identification: GdtPhysicianIdentificationObject, // Obj_Arztidentifikation
    pub object_basic_diagnostics: GdtBasicDiagnosticsObject, // Obj_Basisdiagnostikdia
    pub object_permanent_diagnosis: GdtPermanentDiagnosisObject, // Obj_Dauerdiagnosis
    pub object_permanent_medication: GdtPermanentMedicationObject, // Obj_Dauermedikament
    pub object_diagnosis: GdtDiagnosisObject, // Obj_Diagnosis
    pub object_admission: GdtAdmissionObject, // Obj_Einweisung
    pub object_header_data: GdtHeaderDataObject, // Obj_Kopfdaten
    pub object_patient: GdtPatientObject, // Obj_Patient
    pub object_invoice_recipient: GdtInvoiceRecipientObject, // Obj_RgEmpfänger
    pub object_end_of_record: GdtEndOfRecordObject, // Obj_Satzende
    pub object_certificate: GdtCertificateObject, // Obj_Schein
    pub object_appointment_request: GdtAppointmentRequestObject, // Obj_Terminanfrage
    pub object_referral: GdtReferralObject, // Obj_Ueberweisung
    pub object_health: GdtHealthObject,   // Obj_Versichertenkarte
}

#[derive(Debug, Default)]
pub struct GdtRequestObject {
    date_of_examination: String, // 6200, DDMMYYYY
    time_of_examination: String, // 6201, e.g. 110435
    request_identifier: String,  // 8310
    request_uid: String,         // 8314
}

#[derive(Debug, Default)]
pub struct GdtAnnexObject {}

#[derive(Debug, Default)]
pub struct GdtPhysicianIdentificationObject {}

#[derive(Debug, Default)]
pub struct GdtBasicDiagnosticsObject {
    pub patient_height: String, // 3622, cm
    pub patient_weight: String, // 3623 (but the example in PDF says 3632), kg
}

#[derive(Debug, Default)]
pub struct GdtPermanentDiagnosisObject {}

#[derive(Debug, Default)]
pub struct GdtPermanentMedicationObject {}

#[derive(Debug, Default)]
pub struct GdtDiagnosisObject {}

#[derive(Debug, Default)]
pub struct GdtAdmissionObject {}

#[derive(Debug, Default)]
pub struct GdtHeaderDataObject {
    pub gdt_id_receiver: String, // 8315, GDT-ID of the receiver
    pub gdt_id_sender: String,   // 8316, GDT-ID of the sender
    pub version_gdt: String,     // 9218, Version GDT
}

#[derive(Debug, Default)]
pub struct GdtPatientObject {
    pub patient_number: String,           // 3000
    pub patient_name: String,             // 3101
    pub patient_first_name: String,       // 3102
    pub patient_dob: String,              // 3103, DDMMYYYY
    pub patient_title: String,            // 3104, Titel des Patienten
    pub insurance_number: String,         // 3105, Versichertennummer des Patienten
    pub address: String,                  // 3106, Wohnort des Patienten
    pub street: String,                   // 3107, Straße des Patienten
    pub patient_gender: GdtPatientGender, // 3110
    pub mobile_phone_number: String,      // 3618, Mobiltelefonnummer
    pub email_address: String,            // 3619 Email-Adresse des Patienten
    pub phone_number: String,             // 3626 Telefonnummer des Patienten var alnum 0951 3458 200
}

#[derive(Debug, Default)]
pub struct GdtInvoiceRecipientObject {}

#[derive(Debug, Default)]
pub struct GdtEndOfRecordObject {}

#[derive(Debug, Default)]
pub struct GdtCertificateObject {}

#[derive(Debug, Default)]
pub struct GdtAppointmentRequestObject {}

#[derive(Debug, Default)]
pub struct GdtReferralObject {}

#[derive(Debug, Default)]
pub struct GdtHealthObject {}

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

pub fn parse_file_lines<P>(
    path: P,
) -> Result<impl std::iter::Iterator<Item = Result<RawGdtLine, GdtError>>, GdtError>
where
    P: AsRef<Path>,
{
    let file = File::open(path).map_err(GdtError::IoError)?;
    let reader = BufReader::new(file);

    return Ok(reader.lines().map(|r_str| {
        r_str
            .map_err(GdtError::IoError)
            .and_then(string_to_gdt_line)
    }));
}

fn string_to_gdt_line(str: String) -> Result<RawGdtLine, GdtError> {
    if str.len() < 7 {
        return Err(GdtError::LineTooShort(str));
    }
    let field_id = u32::from_str(&str[3..7])
        .map_err(|e| GdtError::FieldIdentifierNotNumber(String::from(&str), e))?;

    return Ok(RawGdtLine {
        field_identifier: field_id,
        content: String::from(&str[7..]),
    });
}

pub fn parse_file<P>(path: P) -> Result<GdtFile, GdtError>
where
    P: AsRef<Path>,
{
    let mut file = Default::default();
    let mut iter = parse_file_lines(path)?;
    read_record_header(&mut file, &mut iter)?;
    while let Some(r_next_line) = iter.next() {
        match r_next_line {
            Err(e) => error!("Error in line: {:?}", e),
            Ok(RawGdtLine {
                field_identifier: 8200,
                content,
            }) if content.as_str() == "Obj_Anforderung" => {
                file.object_request = read_request_object(&mut iter)?;
            }
            Ok(RawGdtLine {
                field_identifier: 8200,
                content,
            }) if content.as_str() == "Obj_Kopfdaten" => {
                file.object_header_data = read_header_data_object(&mut iter)?;
            }
            Ok(RawGdtLine {
                field_identifier: 8200,
                content,
            }) if content.as_str() == "Obj_Patient" => {
                file.object_patient = read_patient_object(&mut iter)?;
            }
            Ok(RawGdtLine {
                field_identifier: 8200,
                content,
            }) if content.as_str() == "Obj_Basisdiagnostik" => {
                file.object_basic_diagnostics = read_basic_diagnostics_object(&mut iter)?;
            }
            Ok(RawGdtLine {
                field_identifier: 8202,
                ..
            }) => {
                return Ok(file);
            }
            _ => {}
        }
    }
    return Ok(file);
}

type GdtLineIter<'a> = dyn 'a + std::iter::Iterator<Item = Result<RawGdtLine, GdtError>>;

fn read_record_header(file: &mut GdtFile, iter: &mut GdtLineIter) -> Result<(), GdtError> {
    let first_line = iter
        .next()
        .ok_or_else(|| GdtError::LineNotFound("0".to_string()))
        .and_then(|x| x)?;
    let first_line_content = u32::from_str(&first_line.content)
        .map_err(|e| GdtError::NumberExpected(first_line.content, e))?;

    let second_line = iter
        .next()
        .ok_or_else(|| GdtError::LineNotFound("1".to_string()))
        .and_then(|x| x)?;
    let second_line_content = u32::from_str(&second_line.content)
        .map_err(|e| GdtError::NumberExpected(second_line.content, e))?;

    file.record_type = first_line_content;
    file.record_length = second_line_content;
    return Ok(());
}

fn read_request_object(iter: &mut GdtLineIter) -> Result<GdtRequestObject, GdtError> {
    let mut obj: GdtRequestObject = Default::default();
    while let Some(r_next_line) = iter.next() {
        match r_next_line {
            Err(e) => error!("Error in object: {:?}", e),
            Ok(RawGdtLine {
                field_identifier: 6200,
                content,
            }) => {
                obj.date_of_examination = content;
            }
            Ok(RawGdtLine {
                field_identifier: 6201,
                content,
            }) => {
                obj.time_of_examination = content;
            }
            Ok(RawGdtLine {
                field_identifier: 8310,
                content,
            }) => {
                obj.request_identifier = content;
            }
            Ok(RawGdtLine {
                field_identifier: 8314,
                content,
            }) => {
                obj.request_uid = content;
            }
            Ok(RawGdtLine {
                field_identifier: 8201,
                ..
            }) => {
                return Ok(obj);
            }
            _ => {}
        }
    }
    return Ok(obj);
}

fn read_header_data_object(iter: &mut GdtLineIter) -> Result<GdtHeaderDataObject, GdtError> {
    let mut obj: GdtHeaderDataObject = Default::default();
    while let Some(r_next_line) = iter.next() {
        match r_next_line {
            Err(e) => error!("Error in object: {:?}", e),
            Ok(RawGdtLine {
                field_identifier: 8315,
                content,
            }) => {
                obj.gdt_id_receiver = content;
            }
            Ok(RawGdtLine {
                field_identifier: 8316,
                content,
            }) => {
                obj.gdt_id_sender = content;
            }
            Ok(RawGdtLine {
                field_identifier: 9218,
                content,
            }) => {
                obj.version_gdt = content;
            }
            Ok(RawGdtLine {
                field_identifier: 8201,
                ..
            }) => {
                return Ok(obj);
            }
            _ => {}
        }
    }
    return Ok(obj);
}

fn read_patient_object(iter: &mut GdtLineIter) -> Result<GdtPatientObject, GdtError> {
    let mut obj: GdtPatientObject = Default::default();
    while let Some(r_next_line) = iter.next() {
        match r_next_line {
            Err(e) => error!("Error in object: {:?}", e),
            Ok(RawGdtLine {
                field_identifier: 3000,
                content,
            }) => {
                obj.patient_number = content;
            }
            Ok(RawGdtLine {
                field_identifier: 3101,
                content,
            }) => {
                obj.patient_name = content;
            }
            Ok(RawGdtLine {
                field_identifier: 3102,
                content,
            }) => {
                obj.patient_first_name = content;
            }
            Ok(RawGdtLine {
                field_identifier: 3103,
                content,
            }) => {
                obj.patient_dob = content;
            }
            Ok(RawGdtLine {
                field_identifier: 3104,
                content,
            }) => {
                obj.patient_title = content;
            }
            Ok(RawGdtLine {
                field_identifier: 3105,
                content,
            }) => {
                obj.insurance_number = content;
            }
            Ok(RawGdtLine {
                field_identifier: 3106,
                content,
            }) => {
                obj.address = content;
            }
            Ok(RawGdtLine {
                field_identifier: 3107,
                content,
            }) => {
                obj.street = content;
            }
            Ok(RawGdtLine {
                field_identifier: 3618,
                content,
            }) => {
                obj.mobile_phone_number = content;
            }
            Ok(RawGdtLine {
                field_identifier: 3619,
                content,
            }) => {
                obj.email_address = content;
            }
            Ok(RawGdtLine {
                field_identifier: 3626,
                content,
            }) => {
                obj.phone_number = content;
            }
            Ok(RawGdtLine {
                field_identifier: 3110,
                content,
            }) => {
                if content == "1" {
                    obj.patient_gender = GdtPatientGender::Male;
                } else if content == "2" {
                    obj.patient_gender = GdtPatientGender::Female;
                } else {
                    return Err(GdtError::InvalidValue("Gender".to_string(), content));
                }
            }
            Ok(RawGdtLine {
                field_identifier: 8201,
                ..
            }) => {
                return Ok(obj);
            }
            _ => {}
        }
    }
    return Ok(obj);
}

fn read_basic_diagnostics_object(
    iter: &mut GdtLineIter,
) -> Result<GdtBasicDiagnosticsObject, GdtError> {
    let mut obj: GdtBasicDiagnosticsObject = Default::default();
    while let Some(r_next_line) = iter.next() {
        match r_next_line {
            Err(e) => error!("Error in object: {:?}", e),
            Ok(RawGdtLine {
                field_identifier: 3622,
                content,
            }) => {
                obj.patient_height = content;
            }
            Ok(RawGdtLine {
                field_identifier: 3623,
                content,
            }) => {
                obj.patient_weight = content;
            }
            Ok(RawGdtLine {
                field_identifier: 3632,
                content,
            }) => {
                obj.patient_weight = content;
            }
            Ok(RawGdtLine {
                field_identifier: 8201,
                ..
            }) => {
                return Ok(obj);
            }
            _ => {}
        }
    }
    return Ok(obj);
}

fn dcm_date_to_gdt(str: String) -> String {
    // YYYYMMDD -> DDMMYYYY
    let year = &str[0..4];
    let month = &str[4..6];
    let day = &str[6..8];
    return format!("{}{}{}", day, month, year);
}

fn dcm_gender_to_gdt(gender_str: String) -> Option<GdtPatientGender> {
    if gender_str == "M".to_string() {
        return Some(GdtPatientGender::Male);
    } else if gender_str == "F".to_string() {
        return Some(GdtPatientGender::Female);
    }
    return None;
}

pub fn dcm_xml_to_file(events: &Vec<XmlEvent>) -> GdtFile {
    let mut file: GdtFile = Default::default();
    file.object_header_data.version_gdt = "03.00".to_string();

    if let Some(date) = xml_get_study_date(&events) {
        file.object_request.date_of_examination = dcm_date_to_gdt(date);
    }
    if let Some(time) = xml_get_study_time(&events) {
        file.object_request.time_of_examination = time;
    }

    if let Some(id) = xml_get_patient_patient_id(&events) {
        file.object_patient.patient_number = id;
    }

    if let Some(name) = xml_get_patient_name(&events) {
        file.object_patient.patient_name = name;
    }

    if let Some(birth_date) = xml_get_patient_birth_date(&events) {
        file.object_patient.patient_dob = dcm_date_to_gdt(birth_date);
    }
    if let Some(g) = xml_get_patient_sex(&events).and_then(|x| dcm_gender_to_gdt(x)) {
        file.object_patient.patient_gender = g;
    }
    if let Some(weight_kg) = xml_get_patient_weight_kg(&events) {
        file.object_basic_diagnostics.patient_weight = weight_kg;
    }
    if let Some(height_meter) = xml_get_patient_height_meter(&events) {
        if let Ok(num) = f64::from_str(&height_meter) {
            file.object_basic_diagnostics.patient_height = format!("{}", num * 100.0);
        }
    }
    return file;
}

pub fn file_to_string(file: GdtFile) -> String {
    let header = "01380006301\r\n".to_string();
    let (header_lines, header_obj) = obj_header_to_string(file.object_header_data);
    let (patient_lines, patient) = obj_patient_to_string(file.object_patient);
    let (basic_diagnostics_lines, basic_diagnostics) =
        obj_basic_diagnostics_request_to_string(file.object_basic_diagnostics);
    let (request_lines, request) = obj_gdt_request_to_string(file.object_request);

    let total_lines = 1 /* 8000 header */
        + 1 /* 8100 record length */
        + header_lines
        + patient_lines
        + basic_diagnostics_lines
        + request_lines
        + 1 /* 8202 end of record */;
    let end_of_record = line_body_to_gdt_string(format!("8202{}", total_lines));

    let total_length = header.len()
        + 16 /* 8100 record length */
        + header_obj.len()
        + patient.len()
        + basic_diagnostics.len()
        + request.len()
        + end_of_record.len();
    let record_length = line_body_to_gdt_string(format!("8100{:07}", total_length));

    let output = header
        + &record_length
        + &header_obj
        + &patient
        + &basic_diagnostics
        + &request
        + &end_of_record;
    return output;
}

fn obj_header_to_string(obj: GdtHeaderDataObject) -> (usize, String) {
    let mut lines = Vec::new();
    if obj.gdt_id_receiver.len() > 0 {
        lines.push(format!("8315{}", obj.gdt_id_receiver));
    }
    if obj.gdt_id_sender.len() > 0 {
        lines.push(format!("8316{}", obj.gdt_id_sender));
    }
    lines.push("921803.00".to_string());
    return obj_and_lines_to_gdt_string("Obj_Kopfdaten", lines);
}

fn obj_patient_to_string(obj: GdtPatientObject) -> (usize, String) {
    let mut lines = Vec::new();
    if obj.patient_number.len() > 0 {
        lines.push(format!("3000{}", obj.patient_number));
    }
    if obj.patient_name.len() > 0 {
        lines.push(format!("3101{}", obj.patient_name));
    }
    if obj.patient_first_name.len() > 0 {
        lines.push(format!("3102{}", obj.patient_first_name));
    }
    if obj.patient_dob.len() > 0 {
        lines.push(format!("3102{}", obj.patient_dob));
    }
    lines.push(format!(
        "3110{}",
        match obj.patient_gender {
            GdtPatientGender::Male => "1",
            GdtPatientGender::Female => "2",
        }
    ));
    return obj_and_lines_to_gdt_string("Obj_Patient", lines);
}

fn obj_gdt_request_to_string(obj: GdtRequestObject) -> (usize, String) {
    let mut lines = Vec::new();
    if obj.date_of_examination.len() > 0 {
        lines.push(format!("6200{}", obj.date_of_examination));
    }
    if obj.time_of_examination.len() > 0 {
        lines.push(format!("6201{}", obj.time_of_examination));
    }
    if obj.request_identifier.len() > 0 {
        lines.push(format!("8310{}", obj.request_identifier));
    }
    if obj.request_uid.len() > 0 {
        lines.push(format!("8314{}", obj.request_uid));
    }
    return obj_and_lines_to_gdt_string("Obj_Anforderung", lines);
}

fn obj_basic_diagnostics_request_to_string(obj: GdtBasicDiagnosticsObject) -> (usize, String) {
    let mut lines = Vec::new();
    if obj.patient_height.len() > 0 {
        lines.push(format!("3622{}", obj.patient_height));
    }
    if obj.patient_weight.len() > 0 {
        lines.push(format!("3623{}", obj.patient_weight));
    }
    return obj_and_lines_to_gdt_string("Obj_Basisdiagnostik", lines);
}

fn line_body_to_gdt_string(line: String) -> String {
    return format!("{:03}{}\r\n", line.len() + 5, line);
}

fn obj_and_lines_to_gdt_string(obj_name: &str, lines: Vec<String>) -> (usize, String) {
    let mut string = line_body_to_gdt_string(format!("8200{}", obj_name));
    let mut num_field = 1;
    for line in &lines {
        string += &line_body_to_gdt_string(line.clone());
        num_field += 1;
    }
    string += &line_body_to_gdt_string(format!("8201{}", num_field + 1));
    return (&lines.len() + 2, string);
}
