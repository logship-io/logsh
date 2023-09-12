use anyhow::Error;

#[derive(Debug, clap::Args)]
#[clap(about = "Execute a query against a logship server.")]
pub struct UploadCommand {
    schema: String,
    path: String,
}

pub fn execute_upload(args: UploadCommand) -> Result<(), Error> {
    logsh_core::upload::execute(&args.schema, &args.path)?;
    Ok(())
}
