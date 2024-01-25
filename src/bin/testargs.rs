use std::env;

fn main() -> Result<(), std::io::Error> {
    let arg_str = format!("args: {:?}", env::args());
    let mut current_path = std::env::current_exe()?;
    current_path.pop();
    current_path.push("testout");
    std::fs::write(current_path, arg_str)?;
    return Ok(());
}
