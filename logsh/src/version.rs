use anyhow::{anyhow, Error};
use self_update::self_replace;
use std::io::Write;

mod build {
    // Generated during build.
    include!(concat!(env!("OUT_DIR"), "/package_info.gen.rs"));
}

#[derive(Debug, clap::Args)]
#[clap(about = "logsh version information")]
pub struct VersionCommand {
    #[arg(long, help = "Update to the latest release.")]
    update: bool,
}

pub fn version<W: Write>(
    mut write: W,
    command: VersionCommand,
    level: log::LevelFilter,
) -> Result<(), Error> {
    let mut welcome = String::from("\n");
    let level = match level {
        log::LevelFilter::Off | log::LevelFilter::Error => 0,
        _ => level as u32,
    };

    if level > 0 {
        for row in 0..ROWS {
            welcome.push_str(START_SHIP[row]);
            for _ in 0..level {
                welcome.push_str(CONTAINERS[row]);
            }
            welcome.push_str(END_SHIP[row]);
            welcome.push('\n');
        }
    }

    writeln!(
        write,
        concat!(
            "{}{}",
            "Package: {}\n",
            "Version: {}\n",
            "Rust Edition {}\n",
            "© Copyright 2023 - logship LLC\n",
        ),
        LOGSHIP,
        welcome,
        build::NAME,
        build::VERSION,
        build::EDITION,
    )
    .map_err(|e| anyhow!("Failed to write version: {}", e))?;

    if command.update {
        log::info!("Checking for updates...");
        let latest = self_update::backends::github::Update::configure()
            .repo_owner("logsink")
            .repo_name("logsh")
            .bin_name("logsh")
            .show_download_progress(true)
            .current_version(build::VERSION)
            .build()?
            .get_latest_release()?;

        if latest.version == build::VERSION {
            writeln!(
                write,
                "Matching latest version: v{}. You're up to date!",
                build::VERSION
            )?;
            return Ok(());
        }

        let asset = latest.assets.iter().find(|a| {
            if cfg!(windows) {
                a.name == "logsh.exe"
            } else {
                a.name == "logsh"
            }
        });

        if let Some(asset) = asset {
            writeln!(write, "Updating to version: v{}", latest.version)?;
            log::info!("Release Name: {}", latest.name);
            log::info!("Release Date: {}", latest.date);
            match latest.body {
                Some(body) if body.trim().len() > 0 => {
                    log::info!("Release Body: {}", body);
                }
                _ => {}
            };

            log::info!(
                "Release asset discovered: {} at {}",
                asset.name,
                asset.download_url
            );
            let path = tempfile::Builder::new()
                .prefix(&format!("logsh_update_{}_", latest.version))
                .tempdir_in(::std::env::current_dir()?)?;
            let path = path.path().join(&asset.name);
            log::debug!("Temporary asset path: {:?}", path);
            let empty = ::std::fs::File::create(&path)?;

            self_update::Download::from_url(&asset.download_url)
                .set_header(reqwest::header::ACCEPT, "application/octet-stream".parse()?)
                .show_progress(true)
                .download_to(&empty)?;

            self_replace::self_replace(path)?;
        } else {
            log::error!("Could not locate latest assets!");
        }
    }

    Ok(())
}

const LOGSHIP: &str = r"    __                     __     _      
   / /____   ____ _ _____ / /_   (_)____ 
  / // __ \ / __ `// ___// __ \ / // __ \
 / // /_/ // /_/ /(__  )/ / / // // /_/ /
/_/ \____/ \__, //____//_/ /_//_// .___/ 
          /____/                /_/      ";

const ROWS: usize = 7;
const START_SHIP: [&str; ROWS] = [
    "     *    _______",
    "       *_ |   ==|",
    "       ||_|     |",
    "      _||_|     |",
    "     |...........",
    r"     \...........",
    "_,_,~')_,~')_,~')",
];
const CONTAINERS: [&str; ROWS] = [
    "                  ",
    "                  ",
    "[|||||||][|||||||]",
    "[|||||||][|||||||]",
    "..................",
    "..................",
    "_,~')_~')_,~')_~')",
];
const END_SHIP: [&str; ROWS] = [
    "                        ",
    "_                       ",
    "|                       ",
    "|_______                ",
    "..o.../                 ",
    "...../ Welcome aboard.  ",
    "_,~')_,~')_,~')_,~')_,~')",
];
