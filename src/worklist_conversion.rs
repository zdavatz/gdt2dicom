use notify::{recommended_watcher, Event, EventHandler, RecursiveMode, Watcher};
use std::fmt;
use std::fs::{create_dir, read_dir, rename};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use uuid::Uuid;

use crate::dcm_worklist::dcm_xml_to_worklist;
use crate::dcm_xml::{default_dcm_xml, file_to_xml, parse_dcm_xml, DcmError, DcmTransferType};
use crate::gdt::{parse_file, GdtError};

#[derive(Debug)]
pub enum WorklistError {
    IoError(std::io::Error),
    DcmError(DcmError),
    GdtError(GdtError),
    NotifyError(notify::Error),
}

impl fmt::Display for WorklistError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorklistError::IoError(e) => write!(f, "IO: {}", e),
            WorklistError::DcmError(e) => write!(f, "DCM: {}", e),
            WorklistError::GdtError(e) => write!(f, "GdtError: {}", e),
            WorklistError::NotifyError(e) => write!(f, "NotifyError: {}", e),
        }
    }
}

pub struct WorklistConversion {
    pub uuid: Uuid,
    input_watcher: Option<(PathBuf, Box<dyn Watcher + Send>)>,
    pub output_dir_path: Option<PathBuf>,
    pub aetitle: String,
    pub modality: String,
    pub log_sender: mpsc::Sender<String>,
}

impl WorklistConversion {
    pub fn new(log_sender: mpsc::Sender<String>) -> WorklistConversion {
        let uuid = Uuid::new_v4();
        return WorklistConversion {
            uuid: uuid,
            input_watcher: None,
            output_dir_path: None,
            aetitle: "".to_string(),
            modality: "".to_string(),
            log_sender: log_sender,
        };
    }
    pub fn input_dir_path(&self) -> Option<PathBuf> {
        if let Some((p, _)) = &self.input_watcher {
            return Some(p.clone());
        }
        return None;
    }
    pub fn set_input_dir_path(
        &mut self,
        path: Option<PathBuf>,
        self_arc: Arc<Mutex<WorklistConversion>>,
    ) -> Result<(), WorklistError> {
        if self.input_dir_path() == path {
            return Ok(());
        }
        if let Some(new_path) = path {
            if let Some((current_path, w)) = &mut self.input_watcher {
                _ = w.unwatch(&current_path.as_path());
                _ = self
                    .log_sender
                    .send(format!("Unwatching {:?}", &current_path));
            }
            let handler = FSEventHandler {
                conversion: self_arc,
            };
            let mut w = recommended_watcher(handler).map_err(WorklistError::NotifyError)?;
            w.watch(&new_path.as_path(), RecursiveMode::NonRecursive)
                .map_err(WorklistError::NotifyError)?;
            self.input_watcher = Some((new_path, Box::new(w)));
        } else {
            self.input_watcher = None;
        }
        return Ok(());
    }

    pub fn unwatch_input_dir(&mut self) {
        if let Some((current_path, w)) = &mut self.input_watcher {
            _ = w.unwatch(&current_path.as_path());
            println!("Unwatching {:?}", &current_path);
            self.input_watcher = None;
        }
    }

    pub fn scan_folder(&self) -> Result<(), WorklistError> {
        if let (Some((input_dir_path, _)), Some(output_dir_path)) =
            (&self.input_watcher, &self.output_dir_path)
        {
            let processed_folder = {
                let mut p = input_dir_path.clone();
                p.push("processed");
                if !p.is_dir() {
                    _ = self
                        .log_sender
                        .send(format!("Creating processed folder at: {}", &p.display()));
                    create_dir(&p).map_err(WorklistError::IoError)?;
                }
                p
            };
            let entries = read_dir(&input_dir_path).map_err(WorklistError::IoError)?;
            for entry in entries {
                let entry = entry.map_err(WorklistError::IoError)?;
                let path = entry.path();
                if path.is_file() {
                    if path.extension().map(|s| s == "gdt").unwrap_or(false) {
                        let filename = &path.file_name().unwrap().to_str().unwrap();
                        let mut out_file_path = output_dir_path.clone();
                        out_file_path.push(&filename);
                        out_file_path.set_extension("wl");
                        convert_gdt_file(&path.as_path(), &out_file_path)?;

                        let mut processed_path = processed_folder.clone();
                        processed_path.push(&filename);
                        rename(&path, processed_path).map_err(WorklistError::IoError)?;
                    } else {
                        // warn
                    }
                }
            }
        }
        Ok(())
    }
}

struct FSEventHandler {
    pub conversion: Arc<Mutex<WorklistConversion>>,
}

impl EventHandler for FSEventHandler {
    fn handle_event(&mut self, event: notify::Result<Event>) {
        if let Ok(event) = event {
            // println!("Event: {:?}", &event);
            match event.kind {
                notify::event::EventKind::Create(notify::event::CreateKind::File) => {
                    if let std::sync::LockResult::Ok(c) = self.conversion.lock() {
                        if event
                            .paths
                            .iter()
                            .any(|p| p.extension().map(|s| s == "gdt").unwrap_or(false))
                        {
                            let result = c.scan_folder();
                            match result {
                                Err(err) => {
                                    _ = c.log_sender.send(format!("Scan error {:?}", err));
                                }
                                Ok(_) => {}
                            }
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

fn convert_gdt_file(input_path: &Path, output_path: &PathBuf) -> Result<(), WorklistError> {
    let gdt_file = parse_file(input_path).map_err(WorklistError::GdtError)?;
    let dicom_xml_path: Option<PathBuf> = None;
    let xml_events = match dicom_xml_path {
        Some(p) => parse_dcm_xml(&p).map_err(WorklistError::DcmError)?,
        _ => default_dcm_xml(DcmTransferType::LittleEndianExplicit),
    };
    let temp_file = file_to_xml(gdt_file, &xml_events).map_err(WorklistError::DcmError)?;
    return dcm_xml_to_worklist(&temp_file.path(), output_path).map_err(WorklistError::IoError);
}
