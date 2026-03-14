use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(author, version, about = "Greentic red-button listener")]
pub struct Cli {
    /// Preferred locale for runtime messages.
    #[arg(long, global = true)]
    pub locale: Option<String>,

    #[arg(long, global = true)]
    pub vendor_id: Option<u16>,

    #[arg(long, global = true)]
    pub product_id: Option<u16>,

    #[arg(long, global = true)]
    pub key: Option<String>,

    #[arg(long, global = true)]
    pub webhook_url: Option<String>,

    #[arg(long, global = true)]
    pub timeout_ms: Option<u64>,

    #[arg(long, global = true, default_value_t = false)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Doctor,
    ListDevices,
    Once,
    Version,
    #[command(hide = true)]
    I18n {
        #[command(subcommand)]
        command: I18nCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum I18nCommand {
    Validate,
    Status,
}
