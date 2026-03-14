mod cli;
mod config;
mod constants;
mod device;
mod doctor;
mod event;
mod i18n;
mod runtime;
mod suppress;
mod webhook;

use std::process::ExitCode;

use anyhow::{Result, anyhow};
use clap::Parser;
use cli::{Cli, Command, I18nCommand};
use config::Config;
use device::default_backend;
use i18n::{I18n, repo_root, status_from_disk, validate_from_disk};

fn main() -> ExitCode {
    let cli = Cli::parse();

    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<()> {
    let bundle = I18n::load().map_err(|err| anyhow!(err))?;
    let locale = bundle.select_locale(cli.locale.clone());

    match cli.command {
        Some(Command::Version) => {
            println!(
                "{}",
                bundle.tf(
                    &locale,
                    "cli.runtime.version",
                    &[("version", env!("CARGO_PKG_VERSION").to_string())],
                )
            );
            Ok(())
        }
        Some(Command::I18n { command }) => run_i18n_command(&bundle, &locale, command),
        Some(Command::ListDevices) => {
            let config = Config::resolve(&cli)?;
            let backend = default_backend();
            let devices = backend.list_devices()?;
            if devices.is_empty() {
                println!("{}", bundle.t(&locale, "cli.devices.none"));
            } else {
                println!("{}", bundle.t(&locale, "cli.devices.header"));
                for device in devices {
                    println!(
                        "{}",
                        bundle.tf(
                            &locale,
                            "cli.devices.entry",
                            &[
                                ("vendor_id", format!("{:04x}", device.vendor_id)),
                                ("product_id", format!("{:04x}", device.product_id)),
                                (
                                    "name",
                                    device.name.unwrap_or_else(|| "unknown device".to_string()),
                                ),
                                ("backend", device.backend.to_string()),
                            ],
                        )
                    );
                }
            }
            println!(
                "{}",
                bundle.tf(
                    &locale,
                    "cli.runtime.config",
                    &[
                        ("vendor_id", config.vendor_id.to_string()),
                        ("product_id", config.product_id.to_string()),
                        ("key", config.key.as_config_value().to_string()),
                        ("webhook_url", config.webhook_url.to_string()),
                        ("timeout_ms", config.timeout_ms.to_string()),
                    ],
                )
            );
            Ok(())
        }
        Some(Command::Doctor) => {
            let config = Config::resolve(&cli)?;
            let backend = default_backend();
            let report = doctor::run(&config, backend.as_ref())?;
            println!("{}", bundle.t(&locale, "cli.doctor.header"));
            for line in report.config_summary {
                println!(
                    "{}",
                    bundle.tf(&locale, "cli.doctor.config_line", &[("line", line)])
                );
            }
            if report.matching_devices.is_empty() {
                println!("{}", bundle.t(&locale, "cli.doctor.no_matching_devices"));
            } else {
                println!("{}", bundle.t(&locale, "cli.doctor.matching_devices"));
                for line in report.matching_devices {
                    println!(
                        "{}",
                        bundle.tf(&locale, "cli.doctor.device_line", &[("line", line)])
                    );
                }
            }
            match report.press_result {
                Some(result) => println!(
                    "{}",
                    bundle.tf(&locale, "cli.doctor.press_ok", &[("result", result)])
                ),
                None => println!("{}", bundle.t(&locale, "cli.doctor.press_timeout")),
            }
            Ok(())
        }
        Some(Command::Once) => {
            let config = Config::resolve(&cli)?;
            let backend = default_backend();
            println!("{}", bundle.t(&locale, "cli.once.waiting"));
            let payload = runtime::run_once(&config, backend.as_ref())?;
            println!(
                "{}",
                bundle.tf(
                    &locale,
                    "cli.once.sent",
                    &[("timestamp", payload.timestamp.to_rfc3339())],
                )
            );
            Ok(())
        }
        None => {
            let config = Config::resolve(&cli)?;
            let backend = default_backend();
            println!("{}", bundle.t(&locale, "cli.runtime.starting"));
            println!(
                "{}",
                bundle.tf(
                    &locale,
                    "cli.runtime.config",
                    &[
                        ("vendor_id", config.vendor_id.to_string()),
                        ("product_id", config.product_id.to_string()),
                        ("key", config.key.as_config_value().to_string()),
                        ("webhook_url", config.webhook_url.to_string()),
                        ("timeout_ms", config.timeout_ms.to_string()),
                    ],
                )
            );
            runtime::reconnecting_listener(&config, backend.as_ref())
        }
    }
}

fn run_i18n_command(bundle: &I18n, locale: &str, command: I18nCommand) -> Result<()> {
    match command {
        I18nCommand::Status => {
            let root = repo_root().map_err(|err| anyhow!(err))?;
            let report = status_from_disk(root).map_err(|err| anyhow!(err))?;
            if report.is_clean() {
                println!(
                    "{}",
                    bundle.tf(
                        locale,
                        "cli.i18n.status.ok",
                        &[("count", report.locale_count.to_string())],
                    )
                );
            } else {
                if !report.missing_files.is_empty() {
                    println!(
                        "{}",
                        bundle.tf(
                            locale,
                            "cli.i18n.status.missing_files",
                            &[("count", report.missing_files.len().to_string())],
                        )
                    );
                }
                if !report.missing_keys.is_empty() {
                    println!(
                        "{}",
                        bundle.tf(
                            locale,
                            "cli.i18n.status.missing_keys",
                            &[("count", report.missing_keys.len().to_string())],
                        )
                    );
                }
                if !report.extra_keys.is_empty() {
                    println!(
                        "{}",
                        bundle.tf(
                            locale,
                            "cli.i18n.status.extra_keys",
                            &[("count", report.extra_keys.len().to_string())],
                        )
                    );
                }
            }
            Ok(())
        }
        I18nCommand::Validate => {
            let root = repo_root().map_err(|err| anyhow!(err))?;
            let issues = validate_from_disk(root).map_err(|err| anyhow!(err))?;
            if issues.is_empty() {
                println!(
                    "{}",
                    bundle.tf(
                        locale,
                        "cli.i18n.validate.ok",
                        &[("count", bundle.supported().len().to_string())],
                    )
                );
                Ok(())
            } else {
                let details = issues
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n");
                Err(anyhow!(bundle.tf(
                    locale,
                    "cli.i18n.validate.error",
                    &[("details", details)],
                )))
            }
        }
    }
}
