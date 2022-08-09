use crate::get_default_target;
use crate::pgo::CargoCommand;
use cargo_metadata::{Artifact, Message};
use colored::Colorize;
use std::collections::HashMap;
use std::fmt::Write as WriteFmt;
use std::io::Write;
use std::process::{Command, Output};

#[derive(Debug, Default)]
struct CargoArgs {
    filtered: Vec<String>,
    contains_target: bool,
}

/// Run `cargo` command in release mode with the provided RUSTFLAGS and Cargo arguments.
pub fn cargo_command_with_flags(
    command: CargoCommand,
    flags: &str,
    cargo_args: Vec<String>,
) -> anyhow::Result<Output> {
    let mut rustflags = std::env::var("RUSTFLAGS").unwrap_or_default();
    write!(&mut rustflags, " {}", flags).unwrap();

    let mut env = HashMap::default();
    env.insert("RUSTFLAGS".to_string(), rustflags);

    let output = cargo_command(command, cargo_args, env)?;
    if !output.status.success() {
        Err(anyhow::anyhow!(
            "Cargo error ({})\n{}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr).red(),
            cargo_json_output_to_string(&output.stdout)
                .unwrap_or_else(|error| format!("Could not parse Cargo stdout: {}", error))
        ))
    } else {
        Ok(output)
    }
}

fn cargo_json_output_to_string(output: &[u8]) -> anyhow::Result<String> {
    let mut messages = Vec::new();

    for message in Message::parse_stream(output) {
        let message = message?;
        write_metadata_message(&mut messages, message);
    }

    Ok(String::from_utf8(messages)?)
}

/// Run `cargo` command in release mode with the provided env variables and Cargo arguments.
fn cargo_command(
    cargo_cmd: CargoCommand,
    cargo_args: Vec<String>,
    env: HashMap<String, String>,
) -> anyhow::Result<Output> {
    let parsed_args = parse_cargo_args(cargo_args);

    let mut command = Command::new("cargo");
    command.args(&[
        cargo_cmd.to_str(),
        "--release",
        "--message-format",
        "json-diagnostic-rendered-ansi",
    ]);

    // --target is passed to avoid instrumenting build scripts
    // See https://doc.rust-lang.org/rustc/profile-guided-optimization.html#a-complete-cargo-workflow
    if !parsed_args.contains_target {
        let default_target = get_default_target().map_err(|error| {
            anyhow::anyhow!(
                "Unable to find default target triple for your platform: {:?}",
                error
            )
        })?;
        command.args(&["--target", &default_target]);
    }

    for arg in parsed_args.filtered {
        command.arg(arg);
    }
    for (key, value) in env {
        command.env(key, value);
    }
    log::debug!("Executing cargo command: {:?}", command);
    Ok(command.output()?)
}

fn parse_cargo_args(cargo_args: Vec<String>) -> CargoArgs {
    let mut args = CargoArgs::default();

    let mut iterator = cargo_args.into_iter();
    while let Some(arg) = iterator.next() {
        match arg.as_str() {
            // Skip `--release`, we will pass it by ourselves.
            "--release" => {
                log::warn!("Do not pass `--release` manually, it will be added automatically by `cargo-pgo`");
            }
            // Skip `--message-format`, we need it to be JSON.
            "--message-format" => {
                log::warn!("Do not pass `--message-format` manually, it will be added automatically by `cargo-pgo`");
                iterator.next(); // skip flag value
            }
            "--target" => {
                args.contains_target = true;
                args.filtered.push(arg);
            }
            _ => args.filtered.push(arg),
        }
    }
    args
}

pub fn handle_metadata_message(message: Message) {
    write_metadata_message(std::io::stdout().lock(), message);
}

fn write_metadata_message<W: Write>(mut stream: W, message: Message) {
    match message {
        Message::TextLine(line) => {
            log::debug!("TextLine {}", line);
            write!(stream, "{}", line).unwrap();
        }
        Message::CompilerMessage(message) => {
            log::debug!("CompilerMessage {}", message);
            write!(
                stream,
                "{}",
                message.message.rendered.unwrap_or(message.message.message)
            )
            .unwrap();
        }
        _ => {}
    }
}

/// Returns a user-friendly name of an artifact kind.
pub fn get_artifact_kind(artifact: &Artifact) -> &str {
    for kind in &artifact.target.kind {
        match kind.as_str() {
            "bin" => {
                return "binary";
            }
            "bench" => {
                return "benchmark";
            }
            "example" => {
                return "example";
            }
            _ => {}
        }
    }
    "artifact"
}

#[cfg(test)]
mod tests {
    use crate::build::parse_cargo_args;

    #[test]
    fn test_parse_cargo_args_filter_release() {
        let args = parse_cargo_args(vec![
            "foo".to_string(),
            "--release".to_string(),
            "--bar".to_string(),
        ]);
        assert_eq!(args.filtered, vec!["foo".to_string(), "--bar".to_string()]);
    }

    #[test]
    fn test_parse_cargo_args_filter_message_format() {
        let args = parse_cargo_args(vec![
            "foo".to_string(),
            "--message-format".to_string(),
            "json".to_string(),
            "bar".to_string(),
        ]);
        assert_eq!(args.filtered, vec!["foo".to_string(), "bar".to_string()]);
    }

    #[test]
    fn test_parse_cargo_args_find_target() {
        let args = parse_cargo_args(vec![
            "--target".to_string(),
            "x64".to_string(),
            "bar".to_string(),
        ]);
        assert_eq!(
            args.filtered,
            vec!["--target".to_string(), "x64".to_string(), "bar".to_string()]
        );
        assert!(args.contains_target);
    }
}
