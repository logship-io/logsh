use anyhow::anyhow;
use clap::{Subcommand, ValueEnum};
use colored::Colorize;
use logsh_core::config;

use crate::{connect, OutputMode};

#[derive(Subcommand)]
#[clap(visible_alias = "cfg", about = "Configure the logsh CLI.")]
pub enum ConfigCommand {
    #[clap(about = "Locate and validate the logsh config.")]
    Path {
        #[arg(long, help = "Exit with error if no logsh config exists.")]
        exists: bool,
        #[arg(long, help = "Exit with error if an existing logsh config is invalid.")]
        validate: bool,
        #[arg(long, help = "Specify a configuration path.")]
        config_path: Option<String>,
    },
    #[clap(subcommand)]
    Connection(ConfigConnectionCommand),
}

#[derive(Subcommand)]
#[clap(about = "Add or update a connection")]
pub enum AddConnectionCommand {
    #[clap(visible_aliases = ["u", "user"], about = "Add a basic auth connection")]
    Basic {
        #[arg(help = "Connection name.")]
        name: String,
        #[arg(help = "Server Endpoint.")]
        server: Option<String>,
        #[arg(short, long, help = "Username.")]
        username: Option<String>,
        #[arg(short, long, help = "Password.")]
        password: Option<String>,
        #[arg(help = "Set the new connection as default.", default_value = "true")]
        default: Option<bool>,
    },
    #[clap(name = "oauth", about = "Add an oauth connection")]
    OAuth {
        #[arg(help = "Connection name.")]
        name: String,
        #[arg(help = "Server Endpoint.")]
        server: Option<String>,
        #[arg(
            long,
            help = "Set the new connection as default.",
            default_value = "true"
        )]
        default: Option<bool>,
        #[arg(long, help = "Specify an OAuth flow.", default_value = "device")]
        flow: OAuthFlow,
    },
}

#[derive(Clone, Copy, Default, ValueEnum)]
pub enum OAuthFlow {
    #[default]
    Device,
    // Browser,
}

#[derive(Subcommand)]
#[clap(visible_aliases = ["c", "conn"], about = "Configure logsh connections.")]
pub enum ConfigConnectionCommand {
    #[clap(subcommand)]
    Add(AddConnectionCommand),
    #[clap(about = "Authenticate an existing connection")]
    Login {
        #[arg(help = "Connection name.")]
        name: Option<String>,
    },
    #[clap(visible_alias = "ls", about = "List connections")]
    List {
        #[arg(short, long, help = "Output result format")]
        output: Option<OutputMode>,
    },
    #[clap(visible_alias = "rm", about = "Remove connections")]
    Remove {
        #[arg(help = "Connection name.")]
        name: String,
    },
    #[clap(visible_alias = "d", about = "Set the default logsh connection")]
    Default {
        #[arg(help = "Connection name.")]
        name: String,
    },
}

#[derive(Subcommand)]
#[clap(visible_aliases = ["s", "sub"], about = "Configure logsh subscriptions.")]
pub enum ConfigSubscriptionCommand {
    #[clap(visible_alias = "ls", about = "List subscriptions.")]
    List {
        #[arg(short, long, help = "Output result format")]
        output: Option<OutputMode>,
    },
    #[clap(visible_alias = "d", about = "Set the default user subscription.")]
    Default {
        #[arg(help = "Subscription name.")]
        name: String,
    },
}

#[derive(Clone, Copy, ValueEnum)]
pub enum AuthType {
    #[clap(help = "Logship user authentication")]
    Basic,
    #[clap(name = "oauth", help = "OAuth authentication")]
    OAuth,
}

pub(crate) fn execute_config(command: ConfigCommand) -> Result<(), anyhow::Error> {
    match command {
        ConfigCommand::Path {
            config_path,
            exists,
            validate,
        } => {
            log::trace!("Entering execute config path. exists: {exists}. validate: {validate}");
            let path = config_path
                .map(|cfg| {
                    cfg.try_into()
                        .map_err(|err| anyhow!("Invalid --config-path specified: {err}"))
                })
                .unwrap_or(
                    config::get_configuration_path()
                        .map_err(|err| anyhow!("Failed to read configuration path: {err}")),
                )?;
            if exists && !path.exists() {
                return Err(anyhow!(
                    "logsh configuration does not exist at path: {}",
                    path.to_string_lossy()
                ));
            }

            if validate && path.exists() {
                let _cfg = config::load().map_err(|err| {
                    anyhow!(
                        "Invalid configuration at {}: {}",
                        path.to_string_lossy().bright_yellow(),
                        err
                    )
                })?;
            }

            println!("{}", path.display());
            Ok(())
        }

        ConfigCommand::Connection(command) => connect::execute_connect(command),
    }
}
