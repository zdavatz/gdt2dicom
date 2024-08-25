use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug)]
pub struct WorklistConversion {
    pub uuid: Uuid,
    pub input_dir_path: Option<PathBuf>,
    pub output_dir_path: Option<PathBuf>,
    pub aetitle: String,
    pub modality: String,
}

impl WorklistConversion {
    pub fn validate(&self) -> bool {
        if self.input_dir_path.is_some() && self.output_dir_path.is_some() {
            return true;
        }
        return false;
    }
}
