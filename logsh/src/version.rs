use anyhow::{anyhow, Error};
use std::io::Write;

mod build {
    // Generated during build.
    include!(concat!(env!("OUT_DIR"), "/package_info.gen.rs"));
}
pub fn version<W: Write>(mut write: W, level: log::LevelFilter) -> Result<(), Error> {
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
            welcome.push_str("\n");
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
    .map_err(|e| anyhow!("Failed to write version: {}", e))
}

const LOGSHIP: &'static str = r"    __                     __     _      
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
