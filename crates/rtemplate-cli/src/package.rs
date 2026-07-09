use anyhow::{anyhow, Result};

pub fn run_package_generate(write: bool) -> Result<()> {
    let mode = if write { "--write" } else { "--check" };
    let status = std::process::Command::new("cargo")
        .args(["xtask", "generate-provider-surfaces", mode])
        .status()
        .map_err(|error| {
            anyhow!("failed to run cargo xtask generate-provider-surfaces: {error}")
        })?;
    if !status.success() {
        return Err(anyhow!(
            "cargo xtask generate-provider-surfaces {mode} failed with {status}"
        ));
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "ok": true,
            "changed": write,
            "command": "package generate",
            "mode": if write { "write" } else { "check" }
        }))?
    );
    Ok(())
}
