use std::ffi::OsStr;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

use crate::command::exec_command;

pub fn dcm_xml_to_worklist(
    xml_file_path: &Path,
    output_path: &PathBuf,
) -> Result<(), std::io::Error> {
    // This function is same as this bash:
    // $ xml2dcm [xml_file_path] [temp1]
    // $ cmdump [temp1] > [temp2]
    // $ dump2dcm -g [temp2] wklist1.wl

    let temp_dcm_file = NamedTempFile::new()?;
    let temp_dcm_file_path = temp_dcm_file.path();
    exec_command(
        "xml2dcm",
        vec![xml_file_path.as_os_str(), temp_dcm_file_path.as_os_str()],
        true,
    )?;

    let mut temp_dump_file = NamedTempFile::new()?;

    let output = exec_command("dcmdump", vec![temp_dcm_file_path.as_os_str()], false)?;
    std::io::stderr().write_all(&output.stderr).unwrap();
    temp_dump_file.write_all(&output.stdout)?;

    exec_command(
        "dump2dcm",
        vec![
            OsStr::new("-g"),
            temp_dump_file.path().as_os_str(),
            output_path.as_path().as_os_str(),
        ],
        true,
    )?;
    return Ok(());
}
