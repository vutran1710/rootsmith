//! Auto-build WASM plugins from source files.
//!
//! This module provides functionality to automatically compile WASM plugins
//! when they don't exist in the output directory.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// WASM plugin builder.
///
/// Builds a .wasm file from a Rust source file, using the source file's name
/// for the output (e.g., `lib.rs` → `lib.wasm`).
#[derive(Debug, Clone, Default)]
pub struct WasmBuilder;

impl WasmBuilder {
    /// Build a WASM plugin from source file to output directory.
    ///
    /// # Arguments
    /// * `input_path` - Path to source file (e.g., `examples/plugin/src/lib.rs`)
    /// * `output_dir` - Output directory (e.g., `examples/output/`)
    ///
    /// # Returns
    /// Path to the compiled .wasm file (e.g., `examples/output/lib.wasm`)
    ///
    /// # Flow
    /// 1. Extract output name from source file (lib.rs → lib.wasm)
    /// 2. Check if {output_dir}/{name}.wasm exists
    /// 3. If not, build with cargo and copy to output
    /// 4. Return the path to the .wasm file
    pub fn build(input_path: impl AsRef<Path>, output_dir: impl AsRef<Path>) -> Result<PathBuf> {
        let input_path = input_path.as_ref();
        let output_dir = output_dir.as_ref();

        // 1. Get output name from source file stem (lib.rs → lib)
        let output_name = input_path
            .file_stem()
            .and_then(|s| s.to_str())
            .context("Invalid source file name")?;

        // 2. Find plugin directory (containing Cargo.toml)
        let plugin_dir = Self::find_plugin_dir(input_path)?;

        // 3. Get cargo library name for the built artifact
        let cargo_lib_name = Self::extract_cargo_lib_name(&plugin_dir)?;

        // 4. Check if .wasm already exists
        let wasm_path = output_dir.join(format!("{}.wasm", output_name));
        if wasm_path.exists() {
            return Ok(wasm_path);
        }

        // 5. Build the plugin
        Self::build_plugin(&plugin_dir, &cargo_lib_name, output_dir, output_name)?;

        // 6. Verify and return
        if !wasm_path.exists() {
            anyhow::bail!(
                "Build succeeded but .wasm not found at: {}",
                wasm_path.display()
            );
        }

        Ok(wasm_path)
    }

    /// Force rebuild a plugin, even if it already exists.
    pub fn rebuild(input_path: impl AsRef<Path>, output_dir: impl AsRef<Path>) -> Result<PathBuf> {
        let input_path = input_path.as_ref();
        let output_dir = output_dir.as_ref();

        let output_name = input_path
            .file_stem()
            .and_then(|s| s.to_str())
            .context("Invalid source file name")?;

        // Remove existing .wasm if present
        let wasm_path = output_dir.join(format!("{}.wasm", output_name));
        if wasm_path.exists() {
            std::fs::remove_file(&wasm_path)?;
        }

        Self::build(input_path, output_dir)
    }

    /// Find the plugin directory containing Cargo.toml by walking up from source file.
    fn find_plugin_dir(input_path: &Path) -> Result<PathBuf> {
        let mut current = input_path.parent();
        while let Some(dir) = current {
            let cargo_toml = dir.join("Cargo.toml");
            if cargo_toml.exists() {
                return Ok(dir.to_path_buf());
            }
            current = dir.parent();
        }

        anyhow::bail!(
            "Could not find Cargo.toml in parent directories of {}",
            input_path.display()
        )
    }

    /// Extract the library name from Cargo.toml (used for the built .wasm artifact name)
    fn extract_cargo_lib_name(plugin_dir: &Path) -> Result<String> {
        let cargo_toml_path = plugin_dir.join("Cargo.toml");
        let content = std::fs::read_to_string(&cargo_toml_path)
            .with_context(|| format!("Failed to read {}", cargo_toml_path.display()))?;

        let toml: toml::Value = content
            .parse()
            .with_context(|| "Failed to parse Cargo.toml")?;

        // Try [lib].name first, then [package].name
        if let Some(lib_name) = toml
            .get("lib")
            .and_then(|lib| lib.get("name"))
            .and_then(|n| n.as_str())
        {
            return Ok(lib_name.replace('-', "_"));
        }

        let pkg_name = toml
            .get("package")
            .and_then(|pkg| pkg.get("name"))
            .and_then(|n| n.as_str())
            .context("No [package].name in Cargo.toml")?;

        // Rust converts dashes to underscores in library names
        Ok(pkg_name.replace('-', "_"))
    }

    /// Build the plugin using cargo and copy to output directory
    fn build_plugin(
        plugin_dir: &Path,
        cargo_lib_name: &str,
        output_dir: &Path,
        output_name: &str,
    ) -> Result<()> {
        // Ensure output directory exists
        std::fs::create_dir_all(output_dir)?;

        // Build with cargo
        let status = Command::new("cargo")
            .args([
                "build",
                "--release",
                "--target",
                "wasm32-unknown-unknown",
            ])
            .current_dir(plugin_dir)
            .status()
            .context("Failed to run cargo build")?;

        if !status.success() {
            anyhow::bail!("cargo build failed with status: {}", status);
        }

        // Copy .wasm to output directory with the desired name
        let built_wasm = plugin_dir
            .join("target")
            .join("wasm32-unknown-unknown")
            .join("release")
            .join(format!("{}.wasm", cargo_lib_name));

        let dest_wasm = output_dir.join(format!("{}.wasm", output_name));

        std::fs::copy(&built_wasm, &dest_wasm).with_context(|| {
            format!(
                "Failed to copy {} to {}",
                built_wasm.display(),
                dest_wasm.display()
            )
        })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::UpstreamData;
    use crate::wasm_host::WasmPluginHost;

    #[test]
    fn test_build_from_source_file() {
        let wasm_path = WasmBuilder::build("examples/plugin/src/lib.rs", "examples/output")
            .expect("Failed to build");
        assert!(wasm_path.ends_with("lib.wasm"));

        let mut host = WasmPluginHost::load(wasm_path.to_str().unwrap(), None)
            .expect("Failed to load");

        let input = serde_json::json!({
            "user_id": "user_123",
            "event_type": "login",
            "timestamp": 1706500000u64,
            "data": "test"
        });

        let record = host
            .process_to_record(UpstreamData::Json(input))
            .expect("Failed to process");

        assert!(!record.namespace.is_empty());
        assert!(!record.key.is_empty());
    }
}
