use std::fs::File;
use std::io::BufReader;
use std::io::BufRead;
use std::path::Path;
use std::str::FromStr;

#[derive(Debug)]
pub struct RawGdtLine {
    field_identifier: u32,
    content: String,
}

pub fn parse_file<P>(path: P) -> std::io::Result<impl std::iter::Iterator<Item = RawGdtLine>> where P: AsRef<Path> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    return Ok(reader.lines().filter_map(|r_str| r_str.ok()).map(string_to_gdt_line))
}

fn string_to_gdt_line(str: String) -> RawGdtLine {
    return RawGdtLine {
        field_identifier: u32::from_str(&str[3..7]).unwrap_or(0),
        content: String::from(&str[7..]),
    };
}
