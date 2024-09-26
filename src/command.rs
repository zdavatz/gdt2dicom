use std::ffi::OsStr;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::process::Output;
use std::sync::mpsc;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

const CREATE_NO_WINDOW: u32 = 0x08000000;

#[cfg(target_os = "linux")]
pub fn binary_to_path(binary_name: String) -> String {
    return binary_name;
}

#[cfg(target_os = "windows")]
pub fn binary_to_path(binary_name: String) -> String {
    if check_if_binary_exists(&binary_name) {
        return binary_name;
    }
    let mut current_path = std::env::current_exe().unwrap();
    current_path.pop();
    let bin_dir = current_path.join("bin/");

    let prefix = bin_dir.to_str().unwrap();
    let full_path = format!("{prefix}{binary_name}");
    if check_if_binary_exists(&full_path) {
        return full_path;
    }
    return binary_name;
}

#[cfg(target_os = "macos")]
pub fn binary_to_path(binary_name: String) -> String {
    if check_if_binary_exists(&binary_name) {
        return binary_name;
    }
    let mut current_path = std::env::current_exe().unwrap();
    current_path.pop();
    let mac_resource_dir = current_path.join("../Resources/bin/");

    let usual_prefixes = vec![
        mac_resource_dir.to_str().unwrap(),
        "/usr/bin/",
        "/usr/sbin/",
        "/opt/homebrew/bin/",
        "/opt/homebrew/sbin/",
        "/usr/local/bin/",
    ];
    for prefix in usual_prefixes {
        let full_path = format!("{prefix}{binary_name}");
        if check_if_binary_exists(&full_path) {
            return full_path;
        }
    }
    return binary_name;
}

pub fn check_if_binary_exists(path: &str) -> bool {
    let mut command = Command::new(path);
    #[cfg(target_os = "windows")]
    command.creation_flags(CREATE_NO_WINDOW);
    let output = command.output();
    return output.is_ok();
}

pub fn exec_command<I, S>(
    command: &str,
    arguments: I,
    print: bool,
    log_sender: Option<&mpsc::Sender<String>>,
) -> Result<Output, std::io::Error>
where
    I: IntoIterator<Item = S> + Clone,
    S: AsRef<OsStr>,
{
    exec_command_with_env(command, arguments, print, log_sender, Vec::new())
}

pub fn exec_command_with_env<I, S>(
    command: &str,
    arguments: I,
    print: bool,
    log_sender: Option<&mpsc::Sender<String>>,
    envs: Vec<(String, PathBuf)>,
) -> Result<Output, std::io::Error>
where
    I: IntoIterator<Item = S> + Clone,
    S: AsRef<OsStr>,
{
    let a = arguments.clone();
    let full_path = binary_to_path(command.to_string());
    let log = format!(
        "Running: {} {}",
        &full_path,
        &a.into_iter()
            .map(|s| s.as_ref().to_os_string().into_string().unwrap())
            .collect::<Vec<_>>()
            .join(" "),
    );
    if let Some(l) = log_sender {
        _ = l.send(log);
    } else {
        println!("{}", log);
    }
    if !envs.is_empty() {
        let log = format!("Env: {:?}", &envs,);
        if let Some(l) = log_sender {
            _ = l.send(log);
        } else {
            println!("{}", log);
        }
    }
    let mut command = Command::new(full_path);
    command.envs(envs);
    command.args(arguments);
    #[cfg(target_os = "windows")]
    command.creation_flags(CREATE_NO_WINDOW);

    let output = command.output()?;

    if print {
        std::io::stdout().write_all(&output.stdout).unwrap();
        std::io::stderr().write_all(&output.stderr).unwrap();
    }
    return Ok(output);
}
