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
        logsh upload my_table data.tsv --progress\n  \
        logsh upload my_table data.tsv --null-value '##/##/####'"
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

    #[arg(long = "null-value", value_name = "TEXT", action = clap::ArgAction::Append,
        help = "Import fields matching this exact text as null (repeatable; requires server support)")]
    null_values: Vec<String>,
}

pub fn execute_upload(args: UploadCommand, output: Option<OutputMode>) -> Result<(), Error> {
    let cfg = config::load()?;
    let connection = cfg.resolve_current_context()?.connection;
    let is_json = matches!(output, Some(OutputMode::Json | OutputMode::JsonPretty));
    logsh_core::upload::UploadOptions {
        timeout: args.timeout.into(),
        pretty_progress: args.progress && !is_json,
        null_values: &args.null_values,
    }
    .execute(&args.schema, &args.path, &connection)?;
    if is_json {
        crate::fmt::print_json(&serde_json::json!({
            "status": "ok",
            "schema": args.schema,
            "path": args.path,
        }));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct Args {
        #[command(flatten)]
        upload: UploadCommand,
    }

    #[test]
    fn null_markers_are_optional_repeatable_and_accept_empty_text() {
        let ordinary = Args::try_parse_from(["logsh", "table", "input.tsv"]).unwrap();
        assert!(ordinary.upload.null_values.is_empty());
        let args = Args::try_parse_from([
            "logsh",
            "table",
            "input.tsv",
            "--null-value",
            "##/##/####",
            "--null-value",
            "",
        ])
        .unwrap();
        assert_eq!(args.upload.null_values, ["##/##/####", ""]);
    }
}
