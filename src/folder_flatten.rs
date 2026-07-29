use notify::{recommended_watcher, Event, EventHandler, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::fs::{copy, read_dir, remove_file, rename};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};

use crate::error::G2DError;

/// Copies files out of per-exam subfolders into one flat folder.
///
/// Some practice softwares (e.g. VitaByte E-PAT) watch a single folder for new
/// files and cannot descend into subfolders, while imaging devices export one
/// subfolder per exam. This watches the export folder recursively and copies
/// matching files up into the flat folder the practice software reads.
///
/// Files are written with a leading dot (`.gdt2dicom_tmp_…`) and renamed into
/// place afterwards, so watchers that ignore dotfiles never see a partially
/// written file.
pub struct FolderFlatten {
    input_watcher: Option<(PathBuf, Box<dyn Watcher + Send>)>,
    output_dir_path: Option<PathBuf>,
    /// Comma separated, e.g. "jpg,dcm". Empty means every file.
    extensions: Option<String>,
    /// `YYYYMMDD`; subfolders whose name starts with an earlier date are skipped.
    cutoff_date: Option<String>,
    /// Comma separated substrings; a file is skipped when its own name or its
    /// parent folder name contains one of them (e.g. "EM-,Muster").
    exclude_patterns: Option<String>,
    log_sender: mpsc::Sender<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderFlattenState {
    pub input_dir_path: Option<PathBuf>,
    pub output_dir_path: Option<PathBuf>,
    pub extensions: Option<String>,
    pub cutoff_date: Option<String>,
    pub exclude_patterns: Option<String>,
}

const TEMP_PREFIX: &str = ".gdt2dicom_tmp_";

impl FolderFlatten {
    pub fn new(log_sender: mpsc::Sender<String>) -> FolderFlatten {
        return FolderFlatten {
            input_watcher: None,
            output_dir_path: None,
            extensions: None,
            cutoff_date: None,
            exclude_patterns: None,
            log_sender: log_sender,
        };
    }

    pub fn to_state(&self) -> FolderFlattenState {
        FolderFlattenState {
            input_dir_path: self.input_dir_path(),
            output_dir_path: self.output_dir_path.clone(),
            extensions: self.extensions.clone(),
            cutoff_date: self.cutoff_date.clone(),
            exclude_patterns: self.exclude_patterns.clone(),
        }
    }

    pub fn from_state(
        state: &FolderFlattenState,
        log_sender: mpsc::Sender<String>,
    ) -> Arc<Mutex<FolderFlatten>> {
        let mut ff = FolderFlatten::new(log_sender);
        ff.output_dir_path = state.output_dir_path.clone();
        ff.set_extensions_string(state.extensions.clone().unwrap_or("".to_string()));
        ff.set_cutoff_date_string(state.cutoff_date.clone().unwrap_or("".to_string()));
        ff.set_exclude_patterns_string(state.exclude_patterns.clone().unwrap_or("".to_string()));
        let arc = Arc::new(Mutex::new(ff));
        let arc1 = arc.clone();
        let mut ff = arc1.lock().unwrap();
        _ = ff.set_input_dir_path(state.input_dir_path.clone(), arc.clone());
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
        self_arc: Arc<Mutex<FolderFlatten>>,
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
                flatten: self_arc,
                // Recursive: this is the whole point, the exports live in subfolders.
            };
            let mut w = recommended_watcher(handler)?;
            w.watch(&new_path.as_path(), RecursiveMode::Recursive)?;
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

    pub fn output_dir_path(&self) -> Option<PathBuf> {
        self.output_dir_path.clone()
    }

    pub fn set_output_dir_path(&mut self, path: Option<PathBuf>) -> Result<(), G2DError> {
        self.output_dir_path = path;
        self.scan_folder()
    }

    pub fn set_extensions_string(&mut self, value: String) {
        self.extensions = if value.trim().is_empty() {
            None
        } else {
            Some(value)
        };
    }

    pub fn set_cutoff_date_string(&mut self, value: String) {
        self.cutoff_date = if value.trim().is_empty() {
            None
        } else {
            Some(value.trim().to_string())
        };
    }

    pub fn set_exclude_patterns_string(&mut self, value: String) {
        self.exclude_patterns = if value.trim().is_empty() {
            None
        } else {
            Some(value)
        };
    }

    fn wanted_extensions(&self) -> Vec<String> {
        match &self.extensions {
            None => vec![],
            Some(s) => s
                .split(',')
                .map(|e| e.trim().trim_start_matches('.').to_ascii_lowercase())
                .filter(|e| !e.is_empty())
                .collect(),
        }
    }

    fn exclude_list(&self) -> Vec<String> {
        match &self.exclude_patterns {
            None => vec![],
            Some(s) => s
                .split(',')
                .map(|e| e.trim().to_ascii_lowercase())
                .filter(|e| !e.is_empty())
                .collect(),
        }
    }

    /// Walks the input folder recursively and copies every file that is missing
    /// from the output folder. Safe to call repeatedly: files already copied, or
    /// already moved on into the output folder's `processed` subfolder by the
    /// receiving software, are skipped.
    pub fn scan_folder(&self) -> Result<(), G2DError> {
        let (input_dir_path, output_dir_path) = match (&self.input_watcher, &self.output_dir_path) {
            (Some((input_dir_path, _)), Some(output_dir_path)) => (input_dir_path, output_dir_path),
            _ => {
                return Ok(());
            }
        };
        if input_dir_path == output_dir_path {
            _ = self
                .log_sender
                .send("Input and output folder are the same, not flattening anything.".to_string());
            return Ok(());
        }
        let extensions = self.wanted_extensions();
        let excludes = self.exclude_list();
        let mut copied = 0;
        self.scan_dir(
            input_dir_path,
            input_dir_path,
            output_dir_path,
            &extensions,
            &excludes,
            &mut copied,
        )?;
        if copied > 0 {
            _ = self.log_sender.send(format!(
                "Copied {} file(s) to {}",
                copied,
                output_dir_path.display()
            ));
        }
        Ok(())
    }

    fn scan_dir(
        &self,
        dir: &Path,
        input_root: &Path,
        output_dir: &Path,
        extensions: &Vec<String>,
        excludes: &Vec<String>,
        copied: &mut u32,
    ) -> Result<(), G2DError> {
        for entry in read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            if name.starts_with('.') {
                continue;
            }
            if path.is_dir() {
                // Never descend into the folder we are writing to, it may be nested
                // inside the export folder.
                if path == output_dir {
                    continue;
                }
                if self.is_excluded_folder(&name, excludes) {
                    continue;
                }
                self.scan_dir(&path, input_root, output_dir, extensions, excludes, copied)?;
            } else if path.is_file() {
                // Only files inside a subfolder need flattening; anything already
                // sitting in the top level is where it should be.
                if path.parent() == Some(input_root) {
                    continue;
                }
                if self.should_copy(&path, &name, extensions, excludes, output_dir) {
                    match self.copy_file(&path, &name, output_dir) {
                        Ok(()) => *copied += 1,
                        Err(err) => {
                            _ = self.log_sender.send(format!(
                                "Could not copy {}: {}",
                                path.display(),
                                err
                            ));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn is_excluded_folder(&self, folder_name: &str, excludes: &Vec<String>) -> bool {
        let lowered = folder_name.to_ascii_lowercase();
        if excludes.iter().any(|e| lowered.contains(e)) {
            return true;
        }
        // Exam folders are named YYYYMMDD_HHMMSS_<PatId>_<Name>; anything older
        // than the cutoff is history and must not be pushed again.
        if let Some(cutoff) = &self.cutoff_date {
            let date = folder_name
                .split('_')
                .next()
                .unwrap_or("")
                .chars()
                .take(8)
                .collect::<String>();
            if date.len() == 8
                && date.chars().all(|c| c.is_ascii_digit())
                && date.as_str() < cutoff.as_str()
            {
                return true;
            }
        }
        false
    }

    fn should_copy(
        &self,
        path: &Path,
        name: &str,
        extensions: &Vec<String>,
        excludes: &Vec<String>,
        output_dir: &Path,
    ) -> bool {
        if !extensions.is_empty() {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_ascii_lowercase())
                .unwrap_or_default();
            if !extensions.contains(&ext) {
                return false;
            }
        }
        let lowered = name.to_ascii_lowercase();
        if excludes.iter().any(|e| lowered.contains(e)) {
            return false;
        }
        if output_dir.join(name).exists() {
            return false;
        }
        // The receiving software moves uploaded files into `processed`, so a file
        // found there has already been handled and must not be copied again.
        if output_dir.join("processed").join(name).exists() {
            return false;
        }
        true
    }

    fn copy_file(&self, path: &Path, name: &str, output_dir: &Path) -> Result<(), G2DError> {
        let temp_path = output_dir.join(format!("{}{}", TEMP_PREFIX, name));
        let final_path = output_dir.join(name);
        if let Err(err) = copy(path, &temp_path) {
            _ = remove_file(&temp_path);
            return Err(G2DError::IoError(err));
        }
        rename(&temp_path, &final_path)?;
        _ = self.log_sender.send(format!(
            "Copied {} -> {}",
            path.display(),
            final_path.display()
        ));
        Ok(())
    }
}

struct FSEventHandler {
    pub flatten: Arc<Mutex<FolderFlatten>>,
}

impl EventHandler for FSEventHandler {
    fn handle_event(&mut self, event: notify::Result<Event>) {
        if let Ok(event) = event {
            match event.kind {
                notify::event::EventKind::Create(_) | notify::event::EventKind::Modify(_) => {
                    // Ignore our own half written files.
                    if event.paths.iter().all(|p| {
                        p.file_name()
                            .and_then(|n| n.to_str())
                            .map(|n| n.starts_with(TEMP_PREFIX))
                            .unwrap_or(false)
                    }) {
                        return;
                    }
                    if let std::sync::LockResult::Ok(f) = self.flatten.lock() {
                        // Give the exporting software a moment to finish writing.
                        std::thread::sleep(std::time::Duration::from_secs(1));
                        if let Err(err) = f.scan_folder() {
                            _ = f.log_sender.send(format!("Scan error {:?}", err));
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
