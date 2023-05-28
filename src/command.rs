use std::ffi::OsStr;
use std::io::Write;
use std::process::Command;
use std::process::Output;

pub fn exec_command<I, S>(
    command: &str,
    arguments: I,
    print: bool,
) -> Result<Output, std::io::Error>
where
    I: IntoIterator<Item = S> + Clone,
    S: AsRef<OsStr>,
{
    let x = arguments.clone();
    println!(
        "Running: {} {}",
        &command,
        &x.into_iter()
            .map(|s| s.as_ref().to_os_string().into_string().unwrap())
            .collect::<Vec<_>>()
            .join(" "),
    );
    let output = Command::new(command).args(arguments).output()?;
    if print {
        std::io::stdout().write_all(&output.stdout).unwrap();
        std::io::stderr().write_all(&output.stderr).unwrap();
    }
    return Ok(output);
}
