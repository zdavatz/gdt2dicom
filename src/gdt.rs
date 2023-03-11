use std::fs::File;
use std::io::BufReader;
use std::io::BufRead;
use std::path::Path;
use std::str::FromStr;
use std::result::Result;

#[derive(Debug)]
pub struct RawGdtLine {
    field_identifier: u32,
    content: String,
}

#[derive(Debug)]
pub enum GdtError {
    IoError(std::io::Error),
    FieldIdentifierNotNumber(String, std::num::ParseIntError),
    LineTooShort(String),
}

pub fn parse_file<P>(path: P) -> std::io::Result<impl std::iter::Iterator<Item = Result<RawGdtLine, GdtError>>> where P: AsRef<Path> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    return Ok(reader.lines().map(|r_str|
        r_str.map_err(|e| GdtError::IoError(e)).and_then(string_to_gdt_line)
    ))
}

fn string_to_gdt_line(str: String) -> Result<RawGdtLine, GdtError> {
    if str.len() < 7 {
        return Err(GdtError::LineTooShort(str));
    }
    let field_id = u32::from_str(&str[3..7]).map_err(|e| GdtError::FieldIdentifierNotNumber(String::from(&str), e))?;

    return Ok(RawGdtLine {
        field_identifier: field_id,
        content: String::from(&str[7..]),
    });
}
