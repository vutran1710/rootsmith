//! WASM Plugin Builder
//!
//! Provides automatic compilation of .rs plugin files to .wasm.
//! Creates a temporary Cargo project to support derive macros and SDK imports.

use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use anyhow::Context;
use anyhow::Result;

/// Default directory for plugin source files (.rs)
pub const PLUGINS_INPUT_DIR: &str = "plugins/input";

/// Default directory for compiled plugins (.wasm)
pub const PLUGINS_OUTPUT_DIR: &str = "plugins/output";

/// Get or build a WASM plugin from a source file.
///
/// Given a filename like "client.rs", this function:
/// 1. Looks for `plugins/input/client.rs`
/// 2. Checks if `plugins/output/client.wasm` exists and is up-to-date
/// 3. If not, compiles the .rs to .wasm using a temporary Cargo project
/// 4. Returns the path to the .wasm file
pub fn get_or_build_plugin(filename: &str) -> Result<PathBuf> {
    let stem = Path::new(filename)
        .file_stem()
        .ok_or_else(|| anyhow::anyhow!("Invalid filename: {}", filename))?
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid UTF-8 in filename"))?;

    let input_path = PathBuf::from(PLUGINS_INPUT_DIR).join(filename);
    let output_path = PathBuf::from(PLUGINS_OUTPUT_DIR).join(format!("{}.wasm", stem));

    // Verify input exists
    if !input_path.exists() {
        anyhow::bail!(
            "Plugin source not found: {}\nExpected at: {}",
            filename,
            input_path.display()
        );
    }

    // Check if rebuild needed
    if needs_rebuild(&input_path, &output_path)? {
        build_plugin(&input_path, &output_path, stem)?;
    }

    Ok(output_path)
}

/// Check if the .wasm needs to be rebuilt.
fn needs_rebuild(input: &Path, output: &Path) -> Result<bool> {
    if !output.exists() {
        return Ok(true);
    }

    let input_modified = input
        .metadata()
        .context("Failed to read input metadata")?
        .modified()
        .context("Failed to get input modification time")?;

    let output_modified = output
        .metadata()
        .context("Failed to read output metadata")?
        .modified()
        .context("Failed to get output modification time")?;

    Ok(input_modified > output_modified)
}

/// Build a plugin using a temporary Cargo project.
fn build_plugin(input: &Path, output: &Path, name: &str) -> Result<()> {
    // Create output directory if needed
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).context("Failed to create output directory")?;
    }

    // Get absolute paths
    let cwd = std::env::current_dir().context("Failed to get current directory")?;
    let input_abs = cwd.join(input);
    let output_abs = cwd.join(output);
    let sdk_derive_path = cwd.join("wasm_plugin_sdk_derive");
    let infra_path = cwd.join("src/wasm_host/infra.rs");

    // Create temporary directory for the Cargo project
    let temp_dir = tempfile::tempdir().context("Failed to create temp directory")?;
    let project_dir = temp_dir.path();

    // 1. Create Cargo.toml
    let cargo_toml = format!(
        r#"[package]
name = "plugin_{name}"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]
path = "src/lib.rs"

[dependencies]
wasm_plugin_sdk_derive = {{ path = "{sdk_path}" }}

[profile.release]
opt-level = "z"
lto = true
"#,
        name = name,
        sdk_path = sdk_derive_path.display()
    );
    std::fs::write(project_dir.join("Cargo.toml"), cargo_toml)
        .context("Failed to write Cargo.toml")?;

    // 2. Create .cargo/config.toml for memory limits
    let cargo_dir = project_dir.join(".cargo");
    std::fs::create_dir_all(&cargo_dir).context("Failed to create .cargo directory")?;
    let cargo_config = r#"[target.wasm32-unknown-unknown]
rustflags = ["-C", "link-args=--max-memory=134217728"]
"#;
    std::fs::write(cargo_dir.join("config.toml"), cargo_config)
        .context("Failed to write cargo config")?;

    // 3. Create src directory
    let src_dir = project_dir.join("src");
    std::fs::create_dir_all(&src_dir).context("Failed to create src directory")?;

    // 4. Generate lib.rs that includes infra.rs and the user's plugin
    let lib_rs = format!(
        r#"#![no_std]

#[macro_use]
pub mod infra {{
    include!("{}");
}}

#[macro_use]
extern crate wasm_plugin_sdk_derive;

use core::slice;
use infra::sdk::{{DecodeFromEnvelope, ToRecord, ToStandardData, IncomingRecord}};
use infra::{{encode_protobuf, encode_response}};

mod client {{
    use crate::infra::sdk;

    pub use crate::infra::sdk::{{IncomingRecord, ToRecord, ToStandardData}};

    pub trait SDKTrait {{
        fn namespace(&self) -> [u8; 32];
        fn key(&self) -> [u8; 32];
        fn value(&self) -> [u8; 32];
        fn timestamp(&self) -> u64;
    }}

    macro_rules! plugin {{
        (name: $name:expr, api_version: $ver:expr, type Input = $input_type:ty, record_kind: $kind:expr) => {{
            #[no_mangle]
            pub extern "C" fn get_api_version() -> u32 {{
                $ver << 16
            }}
        }};
    }}

    include!("{}");
}}

use client::MyWhateverEventName;

#[no_mangle]
pub extern "C" fn process(ptr: *const u8, len: usize) -> *mut u8 {{
    if ptr.is_null() {{
        return encode_response(1, b"null input pointer");
    }}

    let input = unsafe {{ slice::from_raw_parts(ptr, len) }};

    let event = match MyWhateverEventName::decode_from_json(input) {{
        Some(e) => e,
        None => return encode_response(1, b"failed to decode JSON"),
    }};

    let namespace = event.get_namespace();
    let key = event.get_key();
    let timestamp = event.get_timestamp();
    let value = event.get_value();

    let record = IncomingRecord {{
        namespace,
        key,
        value,
        timestamp,
    }};

    let (protobuf_ptr, protobuf_len) = encode_protobuf(&record);
    let protobuf_slice = unsafe {{ slice::from_raw_parts(protobuf_ptr, protobuf_len) }};

    encode_response(0, protobuf_slice)
}}
"#,
        infra_path.display(),
        input_abs.display()
    );
    std::fs::write(src_dir.join("lib.rs"), lib_rs).context("Failed to write lib.rs")?;

    // 5. Build with cargo
    let status = Command::new("cargo")
        .args(["build", "--target", "wasm32-unknown-unknown", "--release"])
        .current_dir(project_dir)
        .status()
        .context("Failed to execute cargo")?;

    if !status.success() {
        anyhow::bail!("Failed to compile plugin: {}", input.display());
    }

    // 6. Copy the output .wasm file
    let wasm_output = project_dir
        .join("target/wasm32-unknown-unknown/release")
        .join(format!("plugin_{}.wasm", name));

    std::fs::copy(&wasm_output, &output_abs).with_context(|| {
        format!(
            "Failed to copy {} to {}",
            wasm_output.display(),
            output_abs.display()
        )
    })?;

    Ok(())
}

/// Check if the wasm32-unknown-unknown target is installed.
pub fn check_wasm_target() -> Result<bool> {
    let output = Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output()
        .context("Failed to run rustup")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.contains("wasm32-unknown-unknown"))
}

/// Install the wasm32-unknown-unknown target.
pub fn install_wasm_target() -> Result<()> {
    let status = Command::new("rustup")
        .args(["target", "add", "wasm32-unknown-unknown"])
        .status()
        .context("Failed to run rustup")?;

    if !status.success() {
        anyhow::bail!("Failed to install wasm32-unknown-unknown target");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_needs_rebuild_missing_output() {
        let input = Path::new("Cargo.toml"); // exists
        let output = Path::new("nonexistent.wasm"); // doesn't exist
        assert!(needs_rebuild(input, output).unwrap());
    }
}
