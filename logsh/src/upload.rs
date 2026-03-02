use anyhow::Error;
use logsh_core::config;

use crate::{fmt::parse::OptionalDurationArg, OutputMode};

#[derive(Debug, clap::Args)]
#[clap(
    about = "Upload CSV/TSV data files to a Logship schema.",
    long_about = "Upload CSV or TSV files to a Logship inflow endpoint.\n\n\
        The schema determines the target table and the file extension\n\
        determines the content-type (text/csv or text/tab-separated-values).\n\n\
        Examples:\n  \
        logsh upload my_table data.csv\n  \
        logsh upload my_table data.tsv --progress"
)]
pub struct UploadCommand {
    #[arg(help = "Target schema name (table to upload into)")]
    schema: String,

    #[arg(help = "Path to CSV or TSV file to upload")]
    path: String,

    #[arg(
        short,
        long,
        help = "Upload timeout (e.g. '30s', '5m'). Use 'none' to disable.",
        default_value = "none"
    )]
    timeout: OptionalDurationArg,

    #[arg(long, help = "Show upload progress")]
    progress: bool,
}

pub fn execute_upload(args: UploadCommand, output: Option<OutputMode>) -> Result<(), Error> {
    let cfg = config::load()?;
    let connection = cfg
        .contexts
        .get(&cfg.current_context)
        .or_else(|| cfg.contexts.values().next())
        .ok_or(anyhow::anyhow!("Connection does not exist"))?;
    let is_json = matches!(output, Some(OutputMode::Json | OutputMode::JsonPretty));
    logsh_core::upload::execute(
        &args.schema,
        &args.path,
        connection,
        args.timeout.into(),
        args.progress && !is_json,
    )?;
    if is_json {
        crate::fmt::print_json(&serde_json::json!({
            "status": "ok",
            "schema": args.schema,
            "path": args.path,
        }));
    }
    Ok(())
}
