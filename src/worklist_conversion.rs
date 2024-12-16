use chrono::prelude::*;
use notify::{recommended_watcher, Event, EventHandler, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::fs::{create_dir, read_dir, rename, File};
use std::io::{Error, ErrorKind};
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
use tempfile::NamedTempFile;

use crate::command::{exec_command, exec_command_with_env};
use crate::dcm_worklist::dcm_to_worklist;
use crate::dcm_xml::{default_dcm_xml, file_to_xml, DcmTransferType};
use crate::error::G2DError;
use crate::gdt::parse_file;

pub struct WorklistConversion {
    input_watcher: Option<(PathBuf, Box<dyn Watcher + Send>)>,
    worklist_dir_path: Arc<Mutex<Option<PathBuf>>>,
    aetitle: Option<String>,
    modality: Option<String>,
    log_sender: mpsc::Sender<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorklistConversionState {
    pub input_dir_path: Option<PathBuf>,
    pub aetitle: Option<String>,
    pub modality: Option<String>,
}

impl WorklistConversion {
    pub fn new(
        log_sender: mpsc::Sender<String>,
        worklist_dir_path: Arc<Mutex<Option<PathBuf>>>,
    ) -> WorklistConversion {
        return WorklistConversion {
            input_watcher: None,
            worklist_dir_path: worklist_dir_path,
            aetitle: None,
            modality: None,
            log_sender: log_sender,
        };
    }
    pub fn to_state(&self) -> WorklistConversionState {
        let input_dir_path = self.input_watcher.as_ref().map(|(path, _)| path.clone());
        WorklistConversionState {
            input_dir_path: input_dir_path,
            aetitle: self.aetitle.clone(),
            modality: self.modality.clone(),
        }
    }
    pub fn from_state(
        state: &WorklistConversionState,
        log_sender: mpsc::Sender<String>,
        worklist_dir_path: Arc<Mutex<Option<PathBuf>>>,
    ) -> Arc<Mutex<WorklistConversion>> {
        let mut wc = WorklistConversion::new(log_sender, worklist_dir_path);
        wc.set_aetitle_string(state.aetitle.clone().unwrap_or("".to_string()));
        wc.set_modality_string(state.modality.clone().unwrap_or("".to_string()));
        let arc = Arc::new(Mutex::new(wc));
        let arc1 = arc.clone();
        let mut wc = arc1.lock().unwrap();
        _ = wc.set_input_dir_path(state.input_dir_path.clone(), arc.clone());
        return arc;
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
    ) -> Result<(), G2DError> {
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

    pub fn scan_folder(&self) -> Result<(), G2DError> {
        let output_folder_path = self.output_folder()?;
        let (input_dir_path, output_folder) = match (&self.input_watcher, output_folder_path) {
            (Some((input_dir_path, _)), Some(output_dir_path)) => (input_dir_path, output_dir_path),
            _ => {
                return Ok(());
            }
        };
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
                    let filename = convert_gdt_file(
                        Some(&self.log_sender),
                        &path.as_path(),
                        &output_folder,
                        &self.aetitle,
                        &self.modality,
                    )?;

                    let mut processed_path = processed_folder.clone();
                    processed_path.push(&filename);
                    processed_path.set_extension("gdt");
                    rename(&path, processed_path)?;
                } else {
                    _ = self
                        .log_sender
                        .send(format!("Found non-GDT file, ignored: {}", path.display()));
                }
            }
        }
        Ok(())
    }

    fn output_folder(&self) -> Result<Option<PathBuf>, G2DError> {
        let o_worklist_dir = self.worklist_dir_path.lock().unwrap();
        let worklist_dir = match o_worklist_dir.deref() {
            None => {
                return Ok(None);
            }
            Some(a) => a,
        };
        let aetitle = match &self.aetitle {
            None => {
                return Ok(Some(worklist_dir.clone()));
            }
            Some(a) => a,
        };
        let mut result = worklist_dir.clone();
        result.push(aetitle);
        if !result.is_dir() {
            _ = self
                .log_sender
                .send(format!("Creating AETitle folder at: {}", &result.display()));
            create_dir(&result)?;
        }
        let mut lock_file = result.clone();
        lock_file.push(".lockfile");
        if !lock_file.is_file() {
            _ = File::create(lock_file)?;
        }
        return Ok(Some(result));
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
    output_dir: &PathBuf,
    aetitle: &Option<String>,
    modality: &Option<String>,
) -> Result<String, G2DError> {
    let gdt_file = parse_file(input_path)?;
    let local: DateTime<Local> = Local::now();
    let timestamp = local.format("%d.%m.%Y_%H.%M.%S").to_string();
    let filename = format!(
        "{}_{}_{}.wl",
        &gdt_file.object_patient.patient_first_name,
        &gdt_file.object_patient.patient_name,
        timestamp
    );
    let mut output_path = output_dir.clone();
    output_path.push(&filename);

    let xml_events = default_dcm_xml(DcmTransferType::LittleEndianExplicit);
    let temp_file = file_to_xml(gdt_file, &xml_events)?;
    let path = temp_file.path();

    let dcm_file = modify_dcm_file(log_sender, aetitle, modality, &path)?;
    dcm_to_worklist(log_sender, &dcm_file.path(), &output_path)?;

    return Ok(filename);
}

#[cfg(target_os = "windows")]
fn dicom_dic_path() -> PathBuf {
    let mut current_path = std::env::current_exe().unwrap();
    current_path.pop();
    current_path.push("share");
    current_path.push("dicom.dic");
    return current_path;
}

#[cfg(target_os = "macos")]
fn dicom_dic_path() -> PathBuf {
    let mut current_path = std::env::current_exe().unwrap();
    current_path.pop();
    return current_path.join("../Resources/share/dicom.dic");
}

#[cfg(target_os = "linux")]
fn dicom_dic_path() -> PathBuf {
    PathBuf::new()
}

fn modify_dcm_file(
    log_sender: Option<&mpsc::Sender<String>>,
    aetitle: &Option<String>,
    modality: &Option<String>,
    xml_file_path: &Path,
) -> Result<NamedTempFile, G2DError> {
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
        return Err(G2DError::IoError(custom_error));
    }

    let envs = if cfg!(not(target_os = "linux")) {
        vec![("DCMDICTPATH".to_string(), dicom_dic_path())]
    } else {
        vec![]
    };

    let output2 = exec_command_with_env(
        "dcmodify",
        vec![OsStr::new("--gen-stud-uid"), temp_dcm_file_path.as_os_str()],
        true,
        log_sender,
        envs.clone(),
    )?;
    if !output2.status.success() {
        let err_str = std::str::from_utf8(&output2.stderr).unwrap();
        if let Some(log_sender) = log_sender {
            _ = log_sender.send(err_str.to_string());
        }
        let custom_error = Error::new(ErrorKind::Other, err_str);
        return Err(G2DError::IoError(custom_error));
    }

    if let Some(aetitle) = aetitle {
        let output3 = exec_command_with_env(
            "dcmodify",
            vec![
                OsStr::new("-i"),
                OsStr::new(&format!("0032,1060={}", aetitle)),
                temp_dcm_file_path.as_os_str(),
            ],
            true,
            log_sender,
            envs.clone(),
        )?;
        if !output3.status.success() {
            let err_str = std::str::from_utf8(&output3.stderr).unwrap();
            if let Some(log_sender) = log_sender {
                _ = log_sender.send(err_str.to_string());
            }
            let custom_error = Error::new(ErrorKind::Other, err_str);
            return Err(G2DError::IoError(custom_error));
        }
    }

    if let Some(modality) = modality {
        let output4 = exec_command_with_env(
            "dcmodify",
            vec![
                OsStr::new("-i"),
                OsStr::new(&format!("(0040,0100)[0].(0008,0060)={}", modality)),
                temp_dcm_file_path.as_os_str(),
            ],
            false,
            log_sender,
            envs,
        )?;
        if !output4.status.success() {
            let err_str = std::str::from_utf8(&output4.stderr).unwrap();
            if let Some(log_sender) = log_sender {
                _ = log_sender.send(err_str.to_string());
            }
            let custom_error = Error::new(ErrorKind::Other, err_str);
            return Err(G2DError::IoError(custom_error));
        }
    }

    return Ok(temp_dcm_file);
}
