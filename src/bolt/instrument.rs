use cargo_metadata::camino::Utf8PathBuf;
use cargo_metadata::Message;
use colored::Colorize;
use std::path::{Path, PathBuf};

use crate::bolt::bolt_rustflags;
use crate::bolt::env::{find_bolt_env, BoltEnv};
use crate::build::{cargo_command_with_flags, handle_metadata_message};
use crate::cli::cli_format_path;
use crate::pgo::CargoCommand;
use crate::workspace::{get_bolt_directory, get_cargo_workspace};
use crate::{clear_directory, run_command};

#[derive(clap::Parser, Debug)]
#[clap(trailing_var_arg(true))]
pub struct BoltInstrumentArgs {
    /// Additional arguments that will be passed to `cargo build`.
    cargo_args: Vec<String>,
}

pub fn bolt_instrument(args: BoltInstrumentArgs) -> anyhow::Result<()> {
    let config = cargo::Config::default()?;
    let workspace = get_cargo_workspace(&config)?;
    let bolt_dir = get_bolt_directory(&workspace)?;

    let bolt_env = find_bolt_env()?;

    if bolt_dir.exists() {
        log::info!("Profile directory already exists, it will be cleared");
        clear_directory(&bolt_dir)?;
    }

    log::info!("BOLT profiles will be stored into {}", bolt_dir.display());

    let output = cargo_command_with_flags(CargoCommand::Build, bolt_rustflags(), args.cargo_args)?;

    for message in Message::parse_stream(output.stdout.as_slice()) {
        let message = message?;
        match message {
            Message::CompilerArtifact(artifact) => {
                if let Some(executable) = artifact.executable {
                    log::info!(
                        "Binary {} built successfully. It will be now instrumented with BOLT.",
                        artifact.target.name.blue(),
                    );
                    let instrumented_path = instrument_binary(&bolt_env, &executable, &bolt_dir)?;
                    log::info!(
                        "Binary {} instrumented successfully. Now run {} on your workload",
                        artifact.target.name.blue(),
                        cli_format_path(&instrumented_path.display())
                    );
                }
            }
            Message::BuildFinished(res) => {
                if res.success {
                    log::info!(
                        "BOLT instrumentation build finished {}",
                        "successfully".green()
                    );
                } else {
                    log::error!("BOLT instrumentation build has {}", "failed".red());
                }
            }
            _ => handle_metadata_message(message),
        }
    }

    Ok(())
}

/// Instruments a binary using BOLT.
/// If it succeeds, returns the path to the instrumented binary.
fn instrument_binary(
    bolt_env: &BoltEnv,
    path: &Utf8PathBuf,
    profile_dir: &Path,
) -> anyhow::Result<PathBuf> {
    let basename = path
        .as_path()
        .file_stem()
        .expect("Cannot extract executable basename");
    std::fs::create_dir_all(profile_dir.join(basename))?;

    let target_path = path
        .parent()
        .expect("Cannot get parent of compiled binary")
        .join(format!("{}-bolt-instrumented", basename));

    let profile_path = format!(
        "{}/{}/profile",
        profile_dir
            .to_str()
            .expect("Could not get path for profile directory"),
        basename
    );

    run_command(
        &bolt_env.bolt,
        &[
            "-instrument",
            path.as_str(),
            "--instrumentation-file-append-pid",
            "--instrumentation-file",
            &profile_path,
            "-update-debug-sections",
            "-o",
            target_path.as_str(),
        ],
    )?;

    Ok(target_path.into_std_path_buf())
}