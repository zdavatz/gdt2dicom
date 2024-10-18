use crate::command::{exec_command, ChildOutput};
use crate::dcm_xml::{parse_dcm_as_xml, xml_get_patient_patient_id};
use crate::error::G2DError;
use notify::{
    recommended_watcher, Event, EventHandler, RecommendedWatcher, RecursiveMode, Watcher,
};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

pub struct DicomJpegExtraction {
    watcher: RecommendedWatcher,
    dicom_dir: PathBuf,
    log_sender: mpsc::Sender<ChildOutput>,
}

struct FSEventHandler {
    log_sender: mpsc::Sender<ChildOutput>,
    jpeg_dir: PathBuf,
}

impl DicomJpegExtraction {
    pub fn new(
        dicom_dir: PathBuf,
        jpeg_dir: PathBuf,
        log_sender: mpsc::Sender<ChildOutput>,
    ) -> Result<DicomJpegExtraction, G2DError> {
        let handler = FSEventHandler {
            log_sender: log_sender.clone(),
            jpeg_dir: jpeg_dir.clone(),
        };
        let w = recommended_watcher(handler)?;
        Ok(DicomJpegExtraction {
            watcher: w,
            dicom_dir: dicom_dir,
            log_sender: log_sender,
        })
    }
    pub fn start(&mut self) -> Result<(), G2DError> {
        self.watcher
            .watch(&self.dicom_dir.as_path(), RecursiveMode::NonRecursive)?;
        Ok(())
    }
    pub fn stop(&mut self) {
        _ = self.watcher.unwatch(&self.dicom_dir.as_path());
    }
}

impl EventHandler for FSEventHandler {
    fn handle_event(&mut self, event: notify::Result<Event>) {
        if let Ok(event) = event {
            println!("Event: {:?}", &event);
            match event.kind {
                notify::event::EventKind::Create(notify::event::CreateKind::File)
                | notify::event::EventKind::Create(notify::event::CreateKind::Any) => {
                    for p in event.paths {
                        println!("path: {:?}", &p);
                        std::thread::sleep(Duration::from_secs(1));
                        // TODO: log error
                        let result = extract_jpeg_from_dicom(&p, &self.jpeg_dir, &self.log_sender);
                        if let Err(err) = result {
                            _ = self
                                .log_sender
                                .send(ChildOutput::Log(format!("Extraction Error: {}", err)));
                        }
                    }
                }
                _ => {
                    // Skip
                }
            }
        }
    }
}

fn extract_jpeg_from_dicom(
    dicom_path: &PathBuf,
    jpeg_dir_path: &PathBuf,
    log_sender: &mpsc::Sender<ChildOutput>,
) -> Result<(), G2DError> {
    // dcmj2pnm +oj +Wm +Fa ./SCc.1.2.276.0.7230010.3.1.4.0.76429.1685251604.512985 xxx.jpg
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
        _ = log_sender.send(ChildOutput::Log(err_str.to_string()));
    }
    return Ok(());
}
