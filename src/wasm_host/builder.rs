//! WASM Plugin Builder
//!
//! Provides automatic compilation of .rs plugin files to .wasm.
//! Creates a temporary Cargo project and generates DecodeFromEnvelope impls.

use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use anyhow::Context;
use anyhow::Result;

pub const PLUGINS_INPUT_DIR: &str = "plugins/input";
pub const PLUGINS_OUTPUT_DIR: &str = "plugins/output";

pub fn get_or_build_plugin(filename: &str) -> Result<PathBuf> {
    let stem = Path::new(filename)
        .file_stem()
        .ok_or_else(|| anyhow::anyhow!("Invalid filename: {}", filename))?
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid UTF-8 in filename"))?;

    let input_path = PathBuf::from(PLUGINS_INPUT_DIR).join(filename);
    let output_path = PathBuf::from(PLUGINS_OUTPUT_DIR).join(format!("{}.wasm", stem));

    if !input_path.exists() {
        anyhow::bail!(
            "Plugin source not found: {}\nExpected at: {}",
            filename,
            input_path.display()
        );
    }

    if needs_rebuild(&input_path, &output_path)? {
        build_plugin(&input_path, &output_path, stem)?;
    }

    Ok(output_path)
}

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

struct StructInfo {
    name: String,
    fields: Vec<(String, String)>, // (name, type)
}

fn parse_derive_struct(source: &str) -> Option<StructInfo> {
    let mut lines = source.lines().peekable();

    while let Some(line) = lines.next() {
        let trimmed = line.trim();

        if trimmed.contains("#[derive(DecodeFromEnvelope)]") {
            while let Some(next_line) = lines.next() {
                let next_trimmed = next_line.trim();
                if next_trimmed.starts_with("pub struct ") {
                    let struct_start = next_trimmed.strip_prefix("pub struct ")?;
                    let name_end = struct_start.find(|c: char| c == ' ' || c == '{')?;
                    let name = struct_start[..name_end].trim().to_string();

                    let mut fields = Vec::new();
                    for field_line in lines.by_ref() {
                        let field_trimmed = field_line.trim();
                        if field_trimmed == "}" {
                            break;
                        }
                        if field_trimmed.starts_with("pub ") {
                            let field_def = field_trimmed.strip_prefix("pub ")?;
                            if let Some(colon_pos) = field_def.find(':') {
                                let field_name = field_def[..colon_pos].trim().to_string();
                                let type_part = field_def[colon_pos + 1..].trim();
                                let field_type = type_part.trim_end_matches(',').trim().to_string();
                                fields.push((field_name, field_type));
                            }
                        }
                    }

                    return Some(StructInfo { name, fields });
                }
                if !next_trimmed.starts_with("#[") {
                    break;
                }
            }
        }
    }

    None
}

fn strip_derive_attributes(source: &str) -> String {
    source
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.starts_with("#[derive(DecodeFromEnvelope)]")
                && !trimmed.starts_with("#[decode(")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn generate_decode_impl(info: &StructInfo) -> String {
    let mut field_extractions = Vec::new();

    for (field_name, field_type) in &info.fields {
        let extraction = if field_type == "u64" {
            format!(
                "{}: extract_json_u64_field(input_str, \"{}\")?",
                field_name, field_name
            )
        } else {
            format!(
                "{}: extract_json_string_field(input_str, \"{}\")?",
                field_name, field_name
            )
        };
        field_extractions.push(extraction);
    }

    format!(
        r#"
    impl DecodeFromEnvelope for {name} {{
        fn decode_from_json(input: &[u8]) -> Option<Self> {{
            unsafe {{ STRING_OFFSET = 0; }}
            let input_str = core::str::from_utf8(input).ok()?;
            Some(Self {{
                {fields}
            }})
        }}
    }}
"#,
        name = info.name,
        fields = field_extractions.join(",\n                ")
    )
}

fn build_plugin(input: &Path, output: &Path, name: &str) -> Result<()> {
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).context("Failed to create output directory")?;
    }

    let cwd = std::env::current_dir().context("Failed to get current directory")?;
    let input_abs = cwd.join(input);
    let output_abs = cwd.join(output);
    let infra_path = cwd.join("src/wasm_host/infra.rs");

    let plugin_source = std::fs::read_to_string(&input_abs)
        .context("Failed to read plugin source")?;

    let struct_info = parse_derive_struct(&plugin_source)
        .ok_or_else(|| anyhow::anyhow!("No #[derive(DecodeFromEnvelope)] struct found"))?;

    let decode_impl = generate_decode_impl(&struct_info);
    let struct_name = &struct_info.name;
    let stripped_source = strip_derive_attributes(&plugin_source);

    let temp_dir = tempfile::tempdir().context("Failed to create temp directory")?;
    let project_dir = temp_dir.path();

    let cargo_toml = format!(
        r#"[package]
name = "plugin_{name}"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]
path = "src/lib.rs"

[profile.release]
opt-level = "z"
lto = true
"#,
        name = name
    );
    std::fs::write(project_dir.join("Cargo.toml"), cargo_toml)
        .context("Failed to write Cargo.toml")?;

    let cargo_dir = project_dir.join(".cargo");
    std::fs::create_dir_all(&cargo_dir).context("Failed to create .cargo directory")?;
    let cargo_config = r#"[target.wasm32-unknown-unknown]
rustflags = ["-C", "link-args=--max-memory=134217728"]
"#;
    std::fs::write(cargo_dir.join("config.toml"), cargo_config)
        .context("Failed to write cargo config")?;

    let src_dir = project_dir.join("src");
    std::fs::create_dir_all(&src_dir).context("Failed to create src directory")?;

    let client_path = src_dir.join("client_plugin.rs");
    std::fs::write(&client_path, &stripped_source)
        .context("Failed to write client plugin source")?;

    let lib_rs = format!(
        r#"#![no_std]

#[macro_use]
pub mod infra {{
    include!("{infra_path}");
}}

use core::slice;
use infra::sdk::{{DecodeFromEnvelope, ToRecord, ToStandardData, IncomingRecord}};
use infra::{{encode_protobuf, encode_response}};

mod client {{
    use crate::infra::sdk;
    use crate::infra::{{extract_json_string_field, extract_json_u64_field, STRING_OFFSET}};

    pub use crate::infra::sdk::{{IncomingRecord, ToRecord, ToStandardData, DecodeFromEnvelope}};

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

    include!("{client_path}");

    {decode_impl}
}}

use client::{struct_name};

#[no_mangle]
pub extern "C" fn process(ptr: *const u8, len: usize) -> *mut u8 {{
    if ptr.is_null() {{
        return encode_response(1, b"null input pointer");
    }}

    let input = unsafe {{ slice::from_raw_parts(ptr, len) }};

    let event = match {struct_name}::decode_from_json(input) {{
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
        infra_path = infra_path.display(),
        client_path = client_path.display(),
        decode_impl = decode_impl,
        struct_name = struct_name
    );
    std::fs::write(src_dir.join("lib.rs"), lib_rs).context("Failed to write lib.rs")?;

    let status = Command::new("cargo")
        .args(["build", "--target", "wasm32-unknown-unknown", "--release"])
        .current_dir(project_dir)
        .status()
        .context("Failed to execute cargo")?;

    if !status.success() {
        anyhow::bail!("Failed to compile plugin: {}", input.display());
    }

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

pub fn check_wasm_target() -> Result<bool> {
    let output = Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output()
        .context("Failed to run rustup")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.contains("wasm32-unknown-unknown"))
}

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
        let input = Path::new("Cargo.toml");
        let output = Path::new("nonexistent.wasm");
        assert!(needs_rebuild(input, output).unwrap());
    }

    #[test]
    fn test_parse_derive_struct() {
        let source = r#"
#[derive(DecodeFromEnvelope)]
#[decode(from = "json")]
pub struct MyEvent {
    pub id: sdk::String,
    pub ts_ms: u64,
    pub name: sdk::String,
}
"#;
        let info = parse_derive_struct(source).unwrap();
        assert_eq!(info.name, "MyEvent");
        assert_eq!(info.fields.len(), 3);
        assert_eq!(info.fields[0], ("id".to_string(), "sdk::String".to_string()));
        assert_eq!(info.fields[1], ("ts_ms".to_string(), "u64".to_string()));
        assert_eq!(info.fields[2], ("name".to_string(), "sdk::String".to_string()));
    }
}
