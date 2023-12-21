use anyhow::Error;
use logsh_core::config;

use crate::fmt::parse::OptionalDurationArg;

#[derive(Debug, clap::Args)]
#[clap(about = "Upload CSV files to your logship server.")]
pub struct UploadCommand {
    schema: String,
    path: String,

    #[arg(
        short,
        long,
        help = "Upload timeout. Use \"none\" to disable timeout.",
        default_value = "none"
    )]
    timeout: OptionalDurationArg,
}

pub fn execute_upload(args: UploadCommand) -> Result<(), Error> {
    let cfg = config::load()?;
    let connection = cfg
        .connections
        .get(&cfg.default_connection)
        .or_else(|| cfg.connections.values().next())
        .ok_or(anyhow::anyhow!("Connection does not exist"))?;
    logsh_core::upload::execute(&args.schema, &args.path, connection, args.timeout.into())?;
    Ok(())
}
