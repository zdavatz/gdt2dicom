use std::default::Default;
use std::path::PathBuf;

use crate::worklist_conversion::WorklistConversionState;

use serde::{Deserialize, Serialize};
use serde_json::json;

pub type WorklistConversionsState = Vec<WorklistConversionState>;

#[derive(Clone, Serialize, Deserialize)]
pub struct StateFile {
    pub worklist_path: Option<PathBuf>,
    pub conversions: WorklistConversionsState,
    // Option for backward compatibility
    pub dicom_server: Option<DicomServerState>,
}

impl Default for StateFile {
    fn default() -> StateFile {
        StateFile {
            worklist_path: None,
            conversions: Vec::new(),
            dicom_server: Some(DicomServerState::default()),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct DicomServerState {
    pub port: Option<u16>,
}

impl Default for DicomServerState {
    fn default() -> DicomServerState {
        DicomServerState { port: None }
    }
}

pub fn write_state_to_file(state: &StateFile) -> Result<(), std::io::Error> {
    let state_string = json!(state).to_string();
    let mut current_path = std::env::current_exe()?;
    current_path.set_file_name("state.json");
    std::fs::write(current_path, state_string)?;
    Ok(())
}

pub fn read_saved_states() -> Result<StateFile, std::io::Error> {
    let mut current_path = std::env::current_exe()?;
    current_path.set_file_name("state.json");
    if !current_path.is_file() {
        return Ok(StateFile::default());
    }
    let data = std::fs::read(&current_path)?;

    Ok(
        serde_json::from_slice::<StateFile>(&data).unwrap_or_else(|err| {
            println!("Restore error {:?}", err);
            StateFile::default()
        }),
    )
}
