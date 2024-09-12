use notify::{recommended_watcher, Event, EventHandler, RecursiveMode, Watcher};
use serde::ser::{Serialize, SerializeStruct, Serializer};
use std::convert::From;
use std::ffi::OsStr;
use std::fmt;
use std::fs::{create_dir, read_dir, rename};
use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
use tempfile::NamedTempFile;

use crate::command::exec_command;
use crate::dcm_worklist::{dcm_to_worklist, dcm_xml_to_worklist};
use crate::dcm_xml::{default_dcm_xml, file_to_xml, DcmError, DcmTransferType};
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

impl From<std::io::Error> for WorklistError {
    fn from(error: std::io::Error) -> Self {
        WorklistError::IoError(error)
    }
}

impl From<DcmError> for WorklistError {
    fn from(error: DcmError) -> Self {
        WorklistError::DcmError(error)
    }
}

impl From<GdtError> for WorklistError {
    fn from(error: GdtError) -> Self {
        WorklistError::GdtError(error)
    }
}

impl From<notify::Error> for WorklistError {
    fn from(error: notify::Error) -> Self {
        WorklistError::NotifyError(error)
    }
}

pub struct WorklistConversion {
    input_watcher: Option<(PathBuf, Box<dyn Watcher + Send>)>,
    pub output_dir_path: Option<PathBuf>,
    aetitle: Option<String>,
    modality: Option<String>,
    log_sender: mpsc::Sender<String>,
}

pub struct WorklistConversionState {
    pub input_dir_path: Option<PathBuf>,
    pub output_dir_path: Option<PathBuf>,
    pub aetitle: Option<String>,
    pub modality: Option<String>,
}

impl Serialize for WorklistConversionState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut s = serializer.serialize_struct("WorklistConversionState", 3)?;
        s.serialize_field("input_dir_path", &self.input_dir_path)?;
        s.serialize_field("output_dir_path", &self.output_dir_path)?;
        s.serialize_field("aetitle", &self.aetitle)?;
        s.serialize_field("modality", &self.modality)?;
        s.end()
    }
}

impl WorklistConversion {
    pub fn new(log_sender: mpsc::Sender<String>) -> WorklistConversion {
        return WorklistConversion {
            input_watcher: None,
            output_dir_path: None,
            aetitle: None,
            modality: None,
            log_sender: log_sender,
        };
    }
    pub fn to_state(&self) -> WorklistConversionState {
        let input_dir_path = self.input_watcher.as_ref().map(|(path, _)| path.clone());
        WorklistConversionState {
            input_dir_path: input_dir_path,
            output_dir_path: self.output_dir_path.clone(),
            aetitle: self.aetitle.clone(),
            modality: self.modality.clone(),
        }
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
            let mut w = recommended_watcher(handler)?;
            w.watch(&new_path.as_path(), RecursiveMode::NonRecursive)?;
            self.input_watcher = Some((new_path, Box::new(w)));
            self.scan_folder()?;
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

    pub fn set_aetitle_string(&mut self, value: String) {
        if value.len() == 0 {
            self.aetitle = None;
        } else {
            self.aetitle = Some(value);
        }
    }

    pub fn set_modality_string(&mut self, value: String) {
        if value.len() == 0 {
            self.modality = None;
        } else {
            self.modality = Some(value);
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
                    create_dir(&p)?;
                }
                p
            };
            let entries = read_dir(&input_dir_path)?;
            for entry in entries {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() {
                    if path
                        .extension()
                        .map(|s| s.to_ascii_lowercase() == "gdt")
                        .unwrap_or(false)
                    {
                        _ = self
                            .log_sender
                            .send(format!("Processing GDT file: {}", &path.display()));
                        let filename = &path.file_name().unwrap().to_str().unwrap();
                        let mut out_file_path = output_dir_path.clone();
                        out_file_path.push(&filename);
                        out_file_path.set_extension("wl");
                        convert_gdt_file(
                            Some(&self.log_sender),
                            &path.as_path(),
                            &out_file_path,
                            &self.aetitle,
                            &self.modality,
                        )?;

                        let mut processed_path = processed_folder.clone();
                        processed_path.push(&filename);
                        rename(&path, processed_path)?;
                    } else {
                        _ = self
                            .log_sender
                            .send(format!("Found non-GDT file, ignored: {}", path.display()));
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
            println!("Event: {:?}", &event);
            match event.kind {
                notify::event::EventKind::Create(notify::event::CreateKind::File)
                | notify::event::EventKind::Create(notify::event::CreateKind::Any) => {
                    if let std::sync::LockResult::Ok(c) = self.conversion.lock() {
                        _ = c.log_sender.send(format!("Event: {:?}", &event));
                        if event.paths.iter().any(|p| {
                            p.extension()
                                .map(|s| s.to_ascii_lowercase() == "gdt")
                                .unwrap_or(false)
                        }) {
                            #[cfg(target_os = "windows")]
                            std::thread::sleep(Duration::from_secs(1));
                            let result = c.scan_folder();
                            if let Err(err) = result {
                                _ = c.log_sender.send(format!("Scan error {:?}", err));
                            }
                        } else {
                            _ = c.log_sender.send(format!(
                                "Not processing: unrecognised extension. {:?}",
                                event.paths
                            ));
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

fn convert_gdt_file(
    log_sender: Option<&mpsc::Sender<String>>,
    input_path: &Path,
    output_path: &PathBuf,
    aetitle: &Option<String>,
    modality: &Option<String>,
) -> Result<(), WorklistError> {
    let gdt_file = parse_file(input_path)?;
    let xml_events = default_dcm_xml(DcmTransferType::LittleEndianExplicit);
    let temp_file = file_to_xml(gdt_file, &xml_events)?;
    let path = temp_file.path();

    if aetitle.is_some() || modality.is_some() {
        let dcm_file = modify_dcm_file(log_sender, aetitle, modality, &path)?;
        return Ok(dcm_to_worklist(log_sender, &dcm_file.path(), output_path)?);
    } else {
        println!("temp_file {:?}", &path);
        return Ok(dcm_xml_to_worklist(log_sender, &path, output_path)?);
    }
}

fn modify_dcm_file(
    log_sender: Option<&mpsc::Sender<String>>,
    aetitle: &Option<String>,
    modality: &Option<String>,
    xml_file_path: &Path,
) -> Result<NamedTempFile, WorklistError> {
    // This function is same as this bash:
    // $ xml2dcm [xml_file_path] [temp1]
    // $ dcmodify -i "0032,1060=MODALITY"
    // $ dcmodify -i "0032,1060=AETITLE"
    let temp_dcm_file = NamedTempFile::new()?;
    let temp_dcm_file_path = temp_dcm_file.path();
    let output1 = exec_command(
        "xml2dcm",
        vec![xml_file_path.as_os_str(), temp_dcm_file_path.as_os_str()],
        false,
        log_sender,
    )?;
    if !output1.status.success() {
        let err_str = std::str::from_utf8(&output1.stderr).unwrap();
        if let Some(log_sender) = log_sender {
            _ = log_sender.send(err_str.to_string());
        }
        let custom_error = Error::new(ErrorKind::Other, err_str);
        return Err(WorklistError::IoError(custom_error));
    }

    if let Some(aetitle) = aetitle {
        let output2 = exec_command(
            "dcmodify",
            vec![
                OsStr::new("-i"),
                OsStr::new(&format!("0032,1060={}", aetitle)),
                temp_dcm_file_path.as_os_str(),
            ],
            true,
            log_sender,
        )?;
        if !output2.status.success() {
            let err_str = std::str::from_utf8(&output2.stderr).unwrap();
            if let Some(log_sender) = log_sender {
                _ = log_sender.send(err_str.to_string());
            }
            let custom_error = Error::new(ErrorKind::Other, err_str);
            return Err(WorklistError::IoError(custom_error));
        }
    }

    if let Some(modality) = modality {
        let output3 = exec_command(
            "dcmodify",
            vec![
                OsStr::new("-i"),
                OsStr::new(&format!("(0040,0100)[0].(0008,0060)={}", modality)),
                temp_dcm_file_path.as_os_str(),
            ],
            false,
            log_sender,
        )?;
        if !output3.status.success() {
            let err_str = std::str::from_utf8(&output3.stderr).unwrap();
            if let Some(log_sender) = log_sender {
                _ = log_sender.send(err_str.to_string());
            }
            let custom_error = Error::new(ErrorKind::Other, err_str);
            return Err(WorklistError::IoError(custom_error));
        }
    }

    return Ok(temp_dcm_file);
}
