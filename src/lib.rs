pub mod bolt;
mod build;
pub mod check;
pub(crate) mod cli;
pub mod pgo;
pub(crate) mod workspace;

use anyhow::anyhow;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn resolve_binary(path: &Path) -> anyhow::Result<PathBuf> {
    Ok(which::which(path)?)
}

/// Runs a command with the provided arguments and returns its stdout.
fn run_command<S: AsRef<OsStr>>(program: S, args: &[&str]) -> anyhow::Result<String> {
    let mut cmd = Command::new(program);
    for arg in args {
        cmd.arg(arg);
    }
    cmd.stdout(std::process::Stdio::piped());
    Ok(String::from_utf8(cmd.output()?.stdout)?)
}

/// Tries to find the default target triple used for compiling on the current host computer.
pub fn get_default_target() -> anyhow::Result<String> {
    const HOST_FIELD: &str = "host: ";

    // Query rustc for defaults.
    let output = run_command("rustc", &["-vV"])?;

    // Parse the default target from stdout.
    let host = output
        .lines()
        .find(|l| l.starts_with(HOST_FIELD))
        .map(|l| &l[HOST_FIELD.len()..])
        .ok_or_else(|| anyhow!("Failed to parse target from rustc output."))?
        .to_owned();
    Ok(host)
}

/// Clears all files from the directory, if it exists.
fn clear_directory(path: &Path) -> std::io::Result<()> {
    std::fs::remove_dir_all(path)?;
    std::fs::create_dir_all(path)
}
