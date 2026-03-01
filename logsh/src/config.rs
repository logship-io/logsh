use anyhow::anyhow;
use clap::{Subcommand, ValueEnum};
use colored::Colorize;
use logsh_core::config;

use crate::{connect, OutputMode};
use serde::Serialize;

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
    Context(ContextCommand),
}

/// Arguments for `logsh ctx add`.
#[derive(clap::Args)]
#[clap(
    about = "Add a new context",
    long_about = "Add a new context connecting to a logship server.\n\n\
                  Examples:\n  \
                  logsh ctx add https://my.logship.ai\n  \
                  logsh ctx add https://my.logship.ai --name prod\n  \
                  logsh ctx add https://my.logship.ai --pat --token <TOKEN>\n  \
                  logsh ctx add https://my.logship.ai --sso"
)]
pub struct AddContextArgs {
    #[arg(help = "Server endpoint URL.")]
    pub server: String,
    #[arg(short, long, help = "Context name. Defaults to the server hostname.")]
    pub name: Option<String>,
    #[arg(long, help = "Use SSO (OAuth) authentication.", group = "auth_type")]
    pub sso: bool,
    #[arg(
        long,
        help = "Use Personal Access Token authentication.",
        group = "auth_type"
    )]
    pub pat: bool,
    #[arg(short, long, help = "Username (basic auth).")]
    pub username: Option<String>,
    #[arg(short, long, help = "Password (basic auth). Prompted if omitted.")]
    pub password: Option<String>,
    #[arg(long, help = "Read password from stdin.")]
    pub password_stdin: bool,
    #[arg(
        short,
        long,
        env = "LOGSH_PAT_TOKEN",
        help = "Personal Access Token. Can also be set via LOGSH_PAT_TOKEN."
    )]
    pub token: Option<String>,
    #[arg(long, help = "Read token from stdin.")]
    pub token_stdin: bool,
    #[arg(long, help = "OAuth flow type.", default_value = "device")]
    pub oauth_flow: OAuthFlow,
    #[arg(long, help = "Don't set this context as the default.")]
    pub no_default: bool,
}

#[derive(Clone, Copy, Default, ValueEnum)]
pub enum OAuthFlow {
    #[default]
    Device,
    // Browser,
}

#[derive(Subcommand)]
#[clap(visible_aliases = ["ctx"], about = "Manage logsh contexts (server connections).")]
pub enum ContextCommand {
    #[clap(about = "Add a new context")]
    Add(AddContextArgs),
    #[clap(about = "Re-authenticate the current or named context")]
    Login {
        #[arg(help = "Context name.")]
        name: Option<String>,
    },
    #[clap(visible_alias = "ls", about = "List contexts")]
    List,
    #[clap(visible_alias = "rm", about = "Remove a context")]
    Remove {
        #[arg(help = "Context name.")]
        name: String,
    },
    #[clap(about = "Set the current (default) context")]
    Use {
        #[arg(help = "Context name.")]
        name: String,
        #[arg(
            short,
            long,
            help = "Also set the current account for this context (by name)."
        )]
        account: Option<String>,
    },
    #[clap(about = "Show the current context")]
    Current,
}

#[derive(Subcommand)]
#[clap(visible_aliases = ["a", "acc"], about = "Configure logsh accounts.")]
pub enum ConfigAccountCommand {
    #[clap(visible_alias = "ls", about = "List accounts.")]
    List {
        #[arg(short, long, help = "Output result format")]
        output: Option<OutputMode>,
    },
    #[clap(visible_alias = "d", about = "Set the default user account.")]
    Default {
        #[arg(help = "Account name.")]
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

pub(crate) fn execute_config(
    command: ConfigCommand,
    output: Option<OutputMode>,
) -> Result<(), anyhow::Error> {
    match command {
        ConfigCommand::Path {
            config_path,
            exists,
            validate,
        } => {
            log::trace!("Entering execute config path. exists: {exists}. validate: {validate}");
            let path: std::path::PathBuf = config_path.map(|cfg| Ok(cfg.into())).unwrap_or(
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

            match output {
                Some(OutputMode::Json) => {
                    #[derive(Serialize)]
                    struct PathOutput {
                        path: String,
                    }
                    crate::fmt::print_json(&PathOutput {
                        path: path.to_string_lossy().to_string(),
                    });
                }
                Some(OutputMode::JsonPretty) => {
                    #[derive(Serialize)]
                    struct PathOutput {
                        path: String,
                    }
                    crate::fmt::print_json_pretty(&PathOutput {
                        path: path.to_string_lossy().to_string(),
                    });
                }
                _ => println!("{}", path.display()),
            }
            Ok(())
        }

        ConfigCommand::Context(command) => connect::execute_context(command, output),
    }
}
