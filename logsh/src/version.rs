use anyhow::{anyhow, Error};
use self_update::self_replace;
use std::io::{stdin, Write};

mod build {
    // Generated during build.
    include!(concat!(env!("OUT_DIR"), "/package_info.gen.rs"));
}

#[derive(Debug, clap::Args)]
#[clap(about = "logsh version information")]
pub struct VersionCommand {
    #[arg(long, group = "update-g", help = "Update to the latest release version.")]
    update: bool,

    #[arg(
        long,
        group = "update-g", 
        help = "Update to the latest pre-release version."
    )]
    update_prerelease: bool,

    #[arg(
        short,
        long,
        requires = "update-g",
        help = "Use with '--update' to skip approval checks."
    )]
    yes: bool,

    #[arg(
        long,
        env = "LOGSH_UPDATE_REPOSITORY",
        requires = "update-g",
        help = "Custom GitHub repository for updates (format: owner/repo). Can also be set via LOGSH_UPDATE_REPOSITORY environment variable."
    )]
    repo: Option<String>,

}

pub fn version<W: Write>(mut write: W, command: VersionCommand, level: u8) -> Result<(), Error> {
    let mut welcome = String::from("\n");

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

    if command.update || command.update_prerelease {
        log::info!("Checking for updates...");
        
        // Parse custom repository or use default
        let (repo_owner, repo_name) = if let Some(repo) = &command.repo {
            let parts: Vec<&str> = repo.split('/').collect();
            if parts.len() != 2 {
                return Err(anyhow!("Invalid repository format. Use 'owner/repo'"));
            }
            (parts[0].to_string(), parts[1].to_string())
        } else {
            ("logship-io".to_string(), "logsh".to_string())
        };

        log::info!("Using repository: {}/{}", repo_owner, repo_name);
        let updater = self_update::backends::github::Update::configure()
            .repo_owner(&repo_owner)
            .repo_name(&repo_name)
            .bin_name("logsh")
            .show_download_progress(true)
            .current_version(build::VERSION)
            .build()?;
        
        let latest = if command.update_prerelease {
            // For prereleases, get the latest pre-release from "latest-pre" tag
            updater.get_release_version("latest-pre")?
        } else {
            // For stable releases, get the latest non-prerelease
            updater.get_latest_release()?
        };

        if latest.version == build::VERSION {
            let update_type = if command.update_prerelease { "prerelease" } else { "release" };
            writeln!(
                write,
                "Matching latest {} version: v{}. You're up to date!",
                update_type,
                build::VERSION
            )?;
            return Ok(());
        }

        // Determine the target architecture for this binary
        let target = if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
            "x86_64-pc-windows-msvc"
        } else if cfg!(all(target_os = "windows", target_arch = "aarch64")) {
            "aarch64-pc-windows-msvc"
        } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            "x86_64-unknown-linux-gnu"
        } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
            "aarch64-unknown-linux-gnu"
        } else if cfg!(all(target_os = "linux", target_arch = "arm")) {
            "armv7-unknown-linux-gnueabihf"
        } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
            "x86_64-apple-darwin"
        } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            "aarch64-apple-darwin"
        } else {
            return Err(anyhow!("Unsupported platform for self-update"));
        };

        let expected_zip_name = format!("logsh-{}.zip", target);

        let asset = latest.assets.iter().find(|a| {
            a.name == expected_zip_name
        });

        if let Some(asset) = asset {
            log::info!("Release Name: {}", latest.name);
            log::info!("Release Date: {}", latest.date);
            match latest.body {
                Some(body) if !body.trim().is_empty() => {
                    log::info!("Release Body: {}", body);
                }
                _ => {}
            };

            if !command.yes {
                writeln!(
                    write,
                    "Update from version v{} to v{}? [y/n]",
                    build::VERSION,
                    latest.version
                )?;

                let mut buf = String::new();
                _ = stdin().read_line(&mut buf)?;
                match buf.trim().to_lowercase().as_str() {
                    "y" | "yes" => {
                        log::debug!(
                            "Update manually approved to v{}, valid yes response: \"{}\"",
                            latest.version,
                            buf
                        );
                        log::info!("User approved version update to v{}.", latest.version);
                    }
                    "n" | "no" => {
                        log::debug!("Update manually declined, valid no response: \"{}\"", buf);
                        log::info!("User declined logsh version update to v{}.", latest.version);
                        return Ok(());
                    }
                    _ => {
                        log::warn!("User input was trash. Expected 'n', \"no\", 'y', or \"yes\". Received \"{}\"", buf);
                        log::info!("Exiting logsh version update.");
                        return Ok(());
                    }
                };
            }

            log::info!(
                "Release asset discovered: {} at {}",
                asset.name,
                asset.download_url
            );

            let tmp_dir = tempfile::Builder::new()
                .prefix(&format!("logsh_update_{}_", latest.version))
                .tempdir_in(::std::env::current_dir()?)?;

            // Download the archive file
            let archive_file = tmp_dir.path().join(&asset.name);
            let file = ::std::fs::File::create(&archive_file)?;

            self_update::Download::from_url(&asset.download_url)
                .set_header(reqwest::header::ACCEPT, "application/octet-stream".parse()?)
                .show_progress(true)
                .download_to(&file)?;

            // Extract the binary from zip
            self_update::Extract::from_source(&archive_file)
                .archive(self_update::ArchiveKind::Zip)
                .extract_into(&tmp_dir.path())?;

            // Find the extracted binary
            let binary_name = if cfg!(windows) { "logsh.exe" } else { "logsh" };
            let binary_file = tmp_dir.path().join(binary_name);

            // Make executable on Unix systems
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = std::fs::metadata(&binary_file)?.permissions();
                perms.set_mode(0o755);
                std::fs::set_permissions(&binary_file, perms)?;
            }

            self_replace::self_replace(binary_file)?;
        } else {
            return Err(anyhow!("Could not locate latest assets!"));
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
