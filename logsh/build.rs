use anyhow::{anyhow, Error};
use std::io::Write;
use std::{fs::File, path::Path};
use toml::Table;

fn main() -> Result<(), Error> {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");

    let cargo = std::fs::read_to_string("Cargo.toml")
        .map_err(|e| anyhow!("Failed to read Cargo.toml: {}", e))?;
    let cargo: Table =
        toml::from_str(&cargo).map_err(|e| anyhow!("Failed to parse Cargo.toml: {}", e))?;

    let path = std::env::var("OUT_DIR")?;
    let path = format!("{}/package_info.gen.rs", path);
    write_build_info(path, cargo)
        .map_err(|e| anyhow!("Failed to output build info module: {}", e))?;
    Ok(())
}

fn write_build_info<P: AsRef<Path>>(path: P, table: Table) -> Result<(), Error> {
    let path = path.as_ref();
    let mut file = File::create(path).map_err(|e| anyhow!("Failed to create {:?}: {}", path, e))?;
    let mut s = String::new();
    let package = table
        .get("package")
        .and_then(|t| t.as_table())
        .ok_or_else(|| anyhow!("Failed to read [package] table in cargo.toml"))?;
    for (k, v) in package {
        write_string(&mut s, k, &v.to_string())
    }

    file.write_all(s.as_bytes())?;
    Ok(())
}

/// Writes a rust constant string
fn write_string(s: &mut String, key: &'_ str, value: &'_ str) {
    let value = value
        .trim_start_matches('"')
        .trim_end_matches('"')
        .replace('\"', "\\\"");
    let upper = key.to_ascii_uppercase();
    s.push_str(&format!(
        concat!(
            "/// Generated accessor for package.{} from Cargo.toml\n",
            "#[allow(dead_code)]\n",
            "pub const {}: &str = \"{}\";\n"
        ),
        key, upper, value
    ));
}
