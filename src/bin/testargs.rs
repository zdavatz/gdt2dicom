use std::env;
use std::path::PathBuf;

fn main() -> Result<(), std::io::Error> {
    let argStr = format!("args: {:?}", env::args());
    let mut current_path = std::env::current_exe()?;
    current_path.pop();
    current_path.push("testout");
    std::fs::write(current_path, argStr)?;
    return Ok(());
}
