use std::ffi::OsStr;
use std::io::{Error, ErrorKind, Write};
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

use crate::command::exec_command;

pub fn dcm_to_worklist(dcm_file_path: &Path, output_path: &PathBuf) -> Result<(), std::io::Error> {
    // This function is same as this bash:
    // $ dcmdump [temp1] > [temp2]
    // $ dump2dcm -g [temp2] wklist1.wl
    let mut temp_dump_file = NamedTempFile::new()?;

    let output2 = exec_command("dcmdump", vec![dcm_file_path.as_os_str()], false)?;
    std::io::stderr().write_all(&output2.stderr).unwrap();
    temp_dump_file.write_all(&output2.stdout)?;
    if !output2.status.success() {
        let err_str = std::str::from_utf8(&output2.stderr).unwrap();
        let custom_error = Error::new(ErrorKind::Other, err_str);
        return Err(custom_error);
    }

    let output3 = exec_command(
        "dump2dcm",
        vec![
            OsStr::new("-g"),
            temp_dump_file.path().as_os_str(),
            output_path.as_path().as_os_str(),
        ],
        true,
    )?;
    if !output3.status.success() {
        let err_str = std::str::from_utf8(&output3.stderr).unwrap();
        let custom_error = Error::new(ErrorKind::Other, err_str);
        return Err(custom_error);
    }
    return Ok(());
}

pub fn dcm_xml_to_worklist(
    xml_file_path: &Path,
    output_path: &PathBuf,
) -> Result<(), std::io::Error> {
    // This function is same as this bash:
    // $ xml2dcm [xml_file_path] [temp1]
    // $ dcmdump [temp1] > [temp2]
    // $ dump2dcm -g [temp2] wklist1.wl

    let temp_dcm_file = NamedTempFile::new()?;
    let temp_dcm_file_path = temp_dcm_file.path();
    let output1 = exec_command(
        "xml2dcm",
        vec![xml_file_path.as_os_str(), temp_dcm_file_path.as_os_str()],
        true,
    )?;
    if !output1.status.success() {
        let err_str = std::str::from_utf8(&output1.stderr).unwrap();
        let custom_error = Error::new(ErrorKind::Other, err_str);
        return Err(custom_error);
    }

    return dcm_to_worklist(temp_dcm_file_path, output_path);
}
