use crate::command::{exec_command, ChildOutput};
use crate::dcm_xml::{parse_dcm_as_xml, xml_get_patient_patient_id};
use crate::error::G2DError;
use std::path::PathBuf;
use std::sync::mpsc;

pub fn extract_jpeg_from_dicom(
    dicom_path: &PathBuf,
    jpeg_dir_path: &PathBuf,
    log_sender: &mpsc::Sender<ChildOutput>,
) -> Result<(), G2DError> {
    let dcm_xml_events = parse_dcm_as_xml(&dicom_path)?;
    let patient_id = xml_get_patient_patient_id(&dcm_xml_events);
    let patient_id = match patient_id {
        Some(id) => id,
        None => {
            _ = log_sender.send(ChildOutput::Log(
                "Cannot patient id from Dicom file".to_string(),
            ));
            return Ok(());
        }
    };
    // dcmj2pnm +oj +Wm +Fa ./dicomfile xxx.jpg
    let mut output_path = jpeg_dir_path.clone();
    output_path.push(patient_id);
    let output = exec_command(
        "dcmj2pnm",
        vec![
            "+oj",
            "+Wm",
            "+Fa",
            dicom_path.to_str().unwrap(),
            output_path.to_str().unwrap(),
        ],
        true,
        None,
    )?;
    if !output.status.success() {
        let err_str = std::str::from_utf8(&output.stderr).unwrap();
        _ = log_sender.send(ChildOutput::Log(format!(
            "dcmj2pnm: {}",
            err_str.to_string()
        )));
    }
    return Ok(());
}
