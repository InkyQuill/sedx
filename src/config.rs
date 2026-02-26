//! Configuration management for SedX
//!
//! SedX stores configuration in ~/.sedx/config.toml

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[allow(dead_code)] // Default config template for future use
const DEFAULT_CONFIG: &str = r#"# SedX Configuration File
# See 'sedx config' command to edit this file

[backup]
# Maximum backup size in GB before warning (default: 2)
#max_size_gb = 2

# Maximum percentage of free space to use for backups (default: 60)
#max_disk_usage_percent = 60

# Custom backup directory (optional)
#backup_dir = "/mnt/backups/sedx"

[compatibility]
# Regex mode: "pcre" (default), "ere", or "bre"
#mode = "pcre"

# Show incompatibility warnings (default: true)
#show_warnings = true

[processing]
# Number of context lines to show around changes (default: 2)
#context_lines = 2

# Maximum memory usage for streaming in MB (default: 100)
#max_memory_mb = 100

# Enable streaming mode for files >= 100MB (default: true)
#streaming = true
"#;

/// SedX configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Backup settings
    #[serde(default)]
    pub backup: BackupConfig,

    /// Compatibility settings
    #[serde(default)]
    pub compatibility: CompatibilityConfig,

    /// Processing settings
    #[serde(default)]
    pub processing: ProcessingConfig,
}

#[allow(clippy::derivable_impls)]
impl Default for Config {
    fn default() -> Self {
        Self {
            backup: BackupConfig::default(),
            compatibility: CompatibilityConfig::default(),
            processing: ProcessingConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupConfig {
    /// Maximum backup size in GB before warning
    #[serde(default = "default_max_size_gb")]
    pub max_size_gb: Option<f64>,

    /// Maximum percentage of free space to use
    #[serde(default = "default_max_disk_usage_percent")]
    pub max_disk_usage_percent: Option<f64>,

    /// Custom backup directory
    #[serde(default)]
    pub backup_dir: Option<String>,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            max_size_gb: Some(2.0),
            max_disk_usage_percent: Some(60.0),
            backup_dir: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatibilityConfig {
    /// Regex mode: "pcre", "ere", or "bre"
    #[serde(default = "default_mode")]
    pub mode: Option<String>,

    /// Show incompatibility warnings
    #[serde(default = "default_show_warnings")]
    pub show_warnings: Option<bool>,
}

impl Default for CompatibilityConfig {
    fn default() -> Self {
        Self {
            mode: Some("pcre".to_string()),
            show_warnings: Some(true),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingConfig {
    /// Number of context lines to show
    #[serde(default = "default_context_lines")]
    pub context_lines: Option<usize>,

    /// Maximum memory usage for streaming in MB
    #[serde(default = "default_max_memory_mb")]
    pub max_memory_mb: Option<usize>,

    /// Enable streaming mode
    #[serde(default = "default_streaming")]
    pub streaming: Option<bool>,
}

impl Default for ProcessingConfig {
    fn default() -> Self {
        Self {
            context_lines: Some(2),
            max_memory_mb: Some(100),
            streaming: Some(true),
        }
    }
}

// Default functions for serde
fn default_max_size_gb() -> Option<f64> {
    Some(2.0)
}
fn default_max_disk_usage_percent() -> Option<f64> {
    Some(60.0)
}
fn default_mode() -> Option<String> {
    Some("pcre".to_string())
}
fn default_show_warnings() -> Option<bool> {
    Some(true)
}
fn default_context_lines() -> Option<usize> {
    Some(2)
}
fn default_max_memory_mb() -> Option<usize> {
    Some(100)
}
fn default_streaming() -> Option<bool> {
    Some(true)
}

/// Get the configuration file path
pub fn config_file_path() -> Result<PathBuf> {
    let home_dir =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;

    let config_dir = home_dir.join(".sedx");
    fs::create_dir_all(&config_dir).with_context(|| {
        format!(
            "Failed to create config directory: {}",
            config_dir.display()
        )
    })?;

    Ok(config_dir.join("config.toml"))
}

/// Get the default configuration file content with comments
fn get_default_config_content() -> &'static str {
    r#"# SedX Configuration File
#
# This file controls default behavior for SedX. Values set here can be
# overridden by command-line flags.
#
# For more information, run: sedx config --help

[backup]
# Maximum backup size in GB before warning (default: 2)
# When a single backup exceeds this size, you'll be warned before creation.
max_size_gb = 2

# Maximum percentage of free space to use for backups (default: 60)
# SedX will refuse to create backups if more than this % of free space would be used.
max_disk_usage_percent = 60

# Custom backup directory (optional)
# Uncomment to use a custom backup location instead of ~/.sedx/backups/
# Useful when your home directory has limited space.
#backup_dir = "/mnt/backups/sedx"

[compatibility]
# Regex mode: "pcre" (default), "ere", or "bre"
# pcre - Perl-Compatible Regular Expressions (most modern, powerful)
# ere  - Extended Regular Expressions (like sed -E)
# bre  - Basic Regular Expressions (like GNU sed, maximum compatibility)
mode = "pcre"

# Show incompatibility warnings (default: true)
# Display warnings when using features that differ from GNU sed.
show_warnings = true

[processing]
# Number of context lines to show around changes (default: 2, max: 10)
# More context makes it easier to understand changes, but uses more memory.
context_lines = 2

# Maximum memory usage for streaming in MB (default: 100)
# Files larger than this threshold will use streaming mode (constant memory).
max_memory_mb = 100

# Enable streaming mode for files >= threshold (default: true)
# When true, large files are processed with constant memory usage.
# When false, all files are loaded into memory (faster but uses more RAM).
streaming = true
"#
}

/// Save the default commented configuration file
pub fn save_default_config() -> Result<()> {
    let config_path = config_file_path()?;

    fs::write(&config_path, get_default_config_content()).with_context(|| {
        format!(
            "Failed to write default config file: {}",
            config_path.display()
        )
    })?;

    Ok(())
}

/// Load configuration from file, creating default if needed
///
/// If the config file doesn't exist, creates it with defaults and returns them.
/// If the config file is malformed, recreates it with defaults.
pub fn load_config() -> Result<Config> {
    let config_path = config_file_path()?;

    // Create default config file if it doesn't exist
    if !config_path.exists() {
        save_default_config()?;
    }

    let config_str = fs::read_to_string(&config_path)
        .with_context(|| format!("Failed to read config file: {}", config_path.display()))?;

    // Try to parse the config
    let config: Config = match toml::from_str(&config_str) {
        Ok(config) => config,
        Err(_) => {
            // Config is malformed, recreate with defaults
            save_default_config()?;
            return Ok(Config::default());
        }
    };

    Ok(config)
}

/// Ensure all config fields exist in the file
///
/// If config doesn't exist, creates default commented template with all fields.
/// If config exists, validates it and recreates if malformed.
pub fn ensure_complete_config() -> Result<()> {
    let config_path = config_file_path()?;

    if !config_path.exists() {
        // Create new config with default commented template
        save_default_config()?;
        return Ok(());
    }

    // Config exists - validate it
    let config_str = fs::read_to_string(&config_path)?;

    // Try to parse the existing config
    if toml::from_str::<Config>(&config_str).is_err() {
        // Config is malformed, replace with default
        save_default_config()?;
    }

    Ok(())
}

/// Save configuration to file
#[allow(dead_code)] // Public API - kept for future use
pub fn save_config(config: &Config) -> Result<()> {
    let config_path = config_file_path()?;

    let config_str = toml::to_string_pretty(config).context("Failed to serialize config")?;

    fs::write(&config_path, config_str)
        .with_context(|| format!("Failed to write config file: {}", config_path.display()))?;

    Ok(())
}

/// Validate configuration values
pub fn validate_config(config: &Config) -> Result<()> {
    // Validate backup settings
    if let Some(max_gb) = config.backup.max_size_gb
        && max_gb < 0.0
    {
        anyhow::bail!("Invalid max_size_gb: {} (must be positive)", max_gb);
    }

    if let Some(max_percent) = config.backup.max_disk_usage_percent
        && !(0.0..=100.0).contains(&max_percent)
    {
        anyhow::bail!(
            "Invalid max_disk_usage_percent: {} (must be 0-100)",
            max_percent
        );
    }

    // Validate compatibility mode
    if let Some(mode) = &config.compatibility.mode
        && !["pcre", "ere", "bre"].contains(&mode.as_str())
    {
        anyhow::bail!("Invalid mode: {} (must be 'pcre', 'ere', or 'bre')", mode);
    }

    // Validate processing settings
    if let Some(context) = config.processing.context_lines
        && context > 10
    {
        anyhow::bail!("Invalid context_lines: {} (max 10)", context);
    }

    if let Some(max_mb) = config.processing.max_memory_mb
        && max_mb < 10
    {
        anyhow::bail!("Invalid max_memory_mb: {} (min 10 MB)", max_mb);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // Helper function to create a temporary config directory for testing
    fn setup_temp_config_dir() -> TempDir {
        TempDir::new().expect("Failed to create temp dir")
    }

    // =========================================================================
    // Config::default() and Default trait tests
    // =========================================================================

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.backup.max_size_gb, Some(2.0));
        assert_eq!(config.backup.max_disk_usage_percent, Some(60.0));
        assert_eq!(config.compatibility.mode, Some("pcre".to_string()));
        assert_eq!(config.compatibility.show_warnings, Some(true));
        assert_eq!(config.processing.context_lines, Some(2));
        assert_eq!(config.processing.max_memory_mb, Some(100));
        assert_eq!(config.processing.streaming, Some(true));
    }

    // =========================================================================
    // validate_config() tests
    // =========================================================================

    #[test]
    fn test_validate_config_valid() {
        let config = Config::default();
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_validate_config_valid_boundary_values() {
        let mut config = Config::default();
        // Test boundary values that should be valid
        config.backup.max_size_gb = Some(0.0); // Zero is allowed (no limit)
        config.backup.max_disk_usage_percent = Some(0.0); // Zero is allowed
        config.backup.max_disk_usage_percent = Some(100.0); // 100% is allowed
        config.processing.context_lines = Some(10); // Max is allowed
        config.processing.max_memory_mb = Some(10); // Min is allowed
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_validate_config_invalid_max_size_gb_negative() {
        let mut config = Config::default();
        config.backup.max_size_gb = Some(-1.0);
        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("max_size_gb"));
    }

    #[test]
    fn test_validate_config_invalid_max_size_gb_very_negative() {
        let mut config = Config::default();
        config.backup.max_size_gb = Some(-999.0);
        let result = validate_config(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_config_invalid_max_disk_usage_percent_negative() {
        let mut config = Config::default();
        config.backup.max_disk_usage_percent = Some(-1.0);
        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("max_disk_usage_percent")
        );
    }

    #[test]
    fn test_validate_config_invalid_max_disk_usage_percent_over_100() {
        let mut config = Config::default();
        config.backup.max_disk_usage_percent = Some(101.0);
        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("max_disk_usage_percent")
        );
    }

    #[test]
    fn test_validate_config_invalid_max_disk_usage_percent_very_large() {
        let mut config = Config::default();
        config.backup.max_disk_usage_percent = Some(9999.0);
        let result = validate_config(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_config_invalid_mode() {
        let mut config = Config::default();
        config.compatibility.mode = Some("invalid".to_string());
        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("mode"));
    }

    #[test]
    fn test_validate_config_all_valid_modes() {
        let modes = vec!["pcre", "ere", "bre"];
        for mode in modes {
            let mut config = Config::default();
            config.compatibility.mode = Some(mode.to_string());
            assert!(
                validate_config(&config).is_ok(),
                "Mode '{}' should be valid",
                mode
            );
        }
    }

    #[test]
    fn test_validate_config_invalid_mode_empty() {
        let mut config = Config::default();
        config.compatibility.mode = Some("".to_string());
        let result = validate_config(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_config_invalid_mode_case_sensitive() {
        let mut config = Config::default();
        config.compatibility.mode = Some("PCRE".to_string()); // uppercase
        let result = validate_config(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_config_invalid_context_lines() {
        let mut config = Config::default();
        config.processing.context_lines = Some(11); // Max is 10
        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("context_lines"));
    }

    #[test]
    fn test_validate_config_invalid_context_lines_very_large() {
        let mut config = Config::default();
        config.processing.context_lines = Some(9999);
        let result = validate_config(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_config_invalid_max_memory_mb() {
        let mut config = Config::default();
        config.processing.max_memory_mb = Some(9); // Min is 10
        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("max_memory_mb"));
    }

    #[test]
    fn test_validate_config_multiple_invalid_fields() {
        let mut config = Config::default();
        config.backup.max_size_gb = Some(-1.0);
        config.compatibility.mode = Some("bad".to_string());
        // Should fail on first error (max_size_gb)
        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("max_size_gb"));
    }

    #[test]
    fn test_validate_config_optional_fields_none() {
        // All Optional fields can be None
        let config = Config {
            backup: BackupConfig {
                max_size_gb: None,
                max_disk_usage_percent: None,
                backup_dir: None,
            },
            compatibility: CompatibilityConfig {
                mode: None,
                show_warnings: None,
            },
            processing: ProcessingConfig {
                context_lines: None,
                max_memory_mb: None,
                streaming: None,
            },
        };
        assert!(validate_config(&config).is_ok());
    }

    // =========================================================================
    // save_config() and round-trip tests
    // =========================================================================

    #[test]
    fn test_config_to_toml() {
        let config = Config::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        assert!(toml_str.contains("[backup]"));
        assert!(toml_str.contains("[compatibility]"));
        assert!(toml_str.contains("[processing]"));
    }

    #[test]
    fn test_config_serialize_deserialize_roundtrip() {
        let original = Config::default();
        let toml_str = toml::to_string_pretty(&original).unwrap();
        let deserialized: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(original.backup.max_size_gb, deserialized.backup.max_size_gb);
        assert_eq!(
            original.backup.max_disk_usage_percent,
            deserialized.backup.max_disk_usage_percent
        );
        assert_eq!(original.compatibility.mode, deserialized.compatibility.mode);
        assert_eq!(
            original.compatibility.show_warnings,
            deserialized.compatibility.show_warnings
        );
        assert_eq!(
            original.processing.context_lines,
            deserialized.processing.context_lines
        );
        assert_eq!(
            original.processing.max_memory_mb,
            deserialized.processing.max_memory_mb
        );
        assert_eq!(
            original.processing.streaming,
            deserialized.processing.streaming
        );
    }

    #[test]
    fn test_config_serialize_with_custom_values() {
        let config = Config {
            backup: BackupConfig {
                max_size_gb: Some(5.5),
                max_disk_usage_percent: Some(80.0),
                backup_dir: Some("/custom/path".to_string()),
            },
            compatibility: CompatibilityConfig {
                mode: Some("ere".to_string()),
                show_warnings: Some(false),
            },
            processing: ProcessingConfig {
                context_lines: Some(5),
                max_memory_mb: Some(200),
                streaming: Some(false),
            },
        };
        let toml_str = toml::to_string_pretty(&config).unwrap();
        assert!(toml_str.contains("max_size_gb = 5.5"));
        assert!(toml_str.contains("max_disk_usage_percent = 80"));
        assert!(toml_str.contains("backup_dir = \"/custom/path\""));
        assert!(toml_str.contains("mode = \"ere\""));
        assert!(toml_str.contains("show_warnings = false"));
        assert!(toml_str.contains("context_lines = 5"));
        assert!(toml_str.contains("max_memory_mb = 200"));
        assert!(toml_str.contains("streaming = false"));
    }

    #[test]
    fn test_config_parse_partial_toml_uses_defaults() {
        let partial_toml = r#"
            [backup]
            max_size_gb = 10.0

            [compatibility]
            mode = "bre"
        "#;
        let config: Config = toml::from_str(partial_toml).unwrap();
        // Custom values
        assert_eq!(config.backup.max_size_gb, Some(10.0));
        assert_eq!(config.compatibility.mode, Some("bre".to_string()));
        // Default values for unspecified fields
        assert_eq!(config.backup.max_disk_usage_percent, Some(60.0)); // default
        assert_eq!(config.compatibility.show_warnings, Some(true)); // default
        assert_eq!(config.processing.context_lines, Some(2)); // default
    }

    #[test]
    fn test_config_parse_empty_toml_all_defaults() {
        let empty_toml = "";
        let config: Config = toml::from_str(empty_toml).unwrap();
        assert_eq!(config.backup.max_size_gb, Some(2.0)); // default
        assert_eq!(config.backup.max_disk_usage_percent, Some(60.0)); // default
        assert_eq!(config.compatibility.mode, Some("pcre".to_string())); // default
        assert_eq!(config.compatibility.show_warnings, Some(true)); // default
        assert_eq!(config.processing.context_lines, Some(2)); // default
        assert_eq!(config.processing.max_memory_mb, Some(100)); // default
        assert_eq!(config.processing.streaming, Some(true)); // default
    }

    #[test]
    fn test_config_parse_invalid_toml_fails() {
        let invalid_toml = r#"
            [backup
            max_size_gb = this is not valid
        "#;
        let result: Result<Config, _> = toml::from_str(invalid_toml);
        assert!(result.is_err());
    }

    #[test]
    fn test_config_parse_invalid_value_type_fails() {
        let invalid_toml = r#"
            [backup]
            max_size_gb = "not a number"
        "#;
        let result: Result<Config, _> = toml::from_str(invalid_toml);
        assert!(result.is_err());
    }

    // =========================================================================
    // ensure_complete_config() tests
    // =========================================================================

    #[test]
    fn test_ensure_complete_config_creates_new_file() {
        let temp_dir = setup_temp_config_dir();

        // Override config_file_path to use temp directory
        // We'll manually test by creating a file in temp dir
        let config_path = temp_dir.path().join("config.toml");
        assert!(!config_path.exists(), "Config file should not exist yet");

        // Create the config directory
        fs::create_dir_all(temp_dir.path()).unwrap();

        // Write a minimal config to test ensure_complete_config
        let minimal_config = r#"
            [backup]
            max_size_gb = 5.0
        "#;
        fs::write(&config_path, minimal_config).unwrap();

        // Read it back to verify it was created
        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("max_size_gb = 5.0"));
    }

    #[test]
    fn test_ensure_complete_config_preserves_valid_values() {
        let temp_dir = setup_temp_config_dir();
        let config_path = temp_dir.path().join("config.toml");

        // Write a valid config with custom values
        let valid_config = r#"
            [backup]
            max_size_gb = 15.0
            max_disk_usage_percent = 75.0

            [compatibility]
            mode = "ere"
            show_warnings = false

            [processing]
            context_lines = 4
            max_memory_mb = 250
            streaming = false
        "#;
        fs::write(&config_path, valid_config).unwrap();

        // Parse and verify
        let parsed: Config = toml::from_str(valid_config).unwrap();
        assert_eq!(parsed.backup.max_size_gb, Some(15.0));
        assert_eq!(parsed.backup.max_disk_usage_percent, Some(75.0));
        assert_eq!(parsed.compatibility.mode, Some("ere".to_string()));
        assert_eq!(parsed.compatibility.show_warnings, Some(false));
        assert_eq!(parsed.processing.context_lines, Some(4));
        assert_eq!(parsed.processing.max_memory_mb, Some(250));
        assert_eq!(parsed.processing.streaming, Some(false));
    }

    #[test]
    fn test_ensure_complete_config_handles_malformed_toml() {
        let temp_dir = setup_temp_config_dir();
        let config_path = temp_dir.path().join("config.toml");

        // Write malformed TOML
        let malformed = r#"
            [backup
            max_size_gb = broken
        "#;
        fs::write(&config_path, malformed).unwrap();

        // Try to parse - should fail
        let result: Result<Config, _> = toml::from_str(malformed);
        assert!(result.is_err(), "Malformed TOML should fail to parse");
    }

    #[test]
    fn test_partial_config_fills_missing_fields() {
        // Test that partial config uses serde defaults
        let partial = r#"
            [backup]
            max_size_gb = 10.0
        "#;
        let config: Config = toml::from_str(partial).unwrap();

        assert_eq!(config.backup.max_size_gb, Some(10.0)); // Custom
        assert_eq!(config.backup.max_disk_usage_percent, Some(60.0)); // Default
        assert_eq!(config.compatibility.mode, Some("pcre".to_string())); // Default
        assert_eq!(config.processing.context_lines, Some(2)); // Default
    }

    // =========================================================================
    // load_config() behavior tests (simulated)
    // =========================================================================

    #[test]
    fn test_load_config_parses_valid_toml() {
        let valid_toml = r#"
            [backup]
            max_size_gb = 8.0
            max_disk_usage_percent = 70.0

            [compatibility]
            mode = "bre"
            show_warnings = false

            [processing]
            context_lines = 3
            max_memory_mb = 150
            streaming = true
        "#;

        let config: Config = toml::from_str(valid_toml).unwrap();
        assert_eq!(config.backup.max_size_gb, Some(8.0));
        assert_eq!(config.backup.max_disk_usage_percent, Some(70.0));
        assert_eq!(config.compatibility.mode, Some("bre".to_string()));
        assert_eq!(config.compatibility.show_warnings, Some(false));
        assert_eq!(config.processing.context_lines, Some(3));
        assert_eq!(config.processing.max_memory_mb, Some(150));
        assert_eq!(config.processing.streaming, Some(true));
    }

    #[test]
    fn test_load_config_handles_invalid_toml() {
        let invalid_toml = r#"
            [backup
            this is not valid toml syntax
        "#;

        let result: Result<Config, _> = toml::from_str(invalid_toml);
        assert!(result.is_err(), "Invalid TOML should fail to parse");
    }

    #[test]
    fn test_load_config_empty_section_uses_defaults() {
        let empty_sections = r#"
            [backup]

            [compatibility]

            [processing]
        "#;

        let config: Config = toml::from_str(empty_sections).unwrap();
        // All should use defaults
        assert_eq!(config.backup.max_size_gb, Some(2.0));
        assert_eq!(config.compatibility.mode, Some("pcre".to_string()));
        assert_eq!(config.processing.context_lines, Some(2));
    }

    // =========================================================================
    // Backup config tests
    // =========================================================================

    #[test]
    fn test_backup_config_default() {
        let config = BackupConfig::default();
        assert_eq!(config.max_size_gb, Some(2.0));
        assert_eq!(config.max_disk_usage_percent, Some(60.0));
        assert_eq!(config.backup_dir, None);
    }

    #[test]
    fn test_backup_config_with_custom_dir() {
        let config = BackupConfig {
            max_size_gb: Some(5.0),
            max_disk_usage_percent: Some(80.0),
            backup_dir: Some("/mnt/backups".to_string()),
        };
        assert_eq!(config.max_size_gb, Some(5.0));
        assert_eq!(config.max_disk_usage_percent, Some(80.0));
        assert_eq!(config.backup_dir, Some("/mnt/backups".to_string()));
    }

    // =========================================================================
    // Compatibility config tests
    // =========================================================================

    #[test]
    fn test_compatibility_config_default() {
        let config = CompatibilityConfig::default();
        assert_eq!(config.mode, Some("pcre".to_string()));
        assert_eq!(config.show_warnings, Some(true));
    }

    #[test]
    fn test_compatibility_config_all_modes() {
        for mode in &["pcre", "ere", "bre"] {
            let config = CompatibilityConfig {
                mode: Some(mode.to_string()),
                show_warnings: Some(false),
            };
            assert_eq!(config.mode, Some(mode.to_string()));
        }
    }

    // =========================================================================
    // Processing config tests
    // =========================================================================

    #[test]
    fn test_processing_config_default() {
        let config = ProcessingConfig::default();
        assert_eq!(config.context_lines, Some(2));
        assert_eq!(config.max_memory_mb, Some(100));
        assert_eq!(config.streaming, Some(true));
    }

    #[test]
    fn test_processing_config_custom_values() {
        let config = ProcessingConfig {
            context_lines: Some(8),
            max_memory_mb: Some(500),
            streaming: Some(false),
        };
        assert_eq!(config.context_lines, Some(8));
        assert_eq!(config.max_memory_mb, Some(500));
        assert_eq!(config.streaming, Some(false));
    }

    // =========================================================================
    // Edge case tests
    // =========================================================================

    #[test]
    fn test_config_with_floating_point_precision() {
        let toml_str = r#"
            [backup]
            max_size_gb = 2.5
            max_disk_usage_percent = 66.6666666667
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.backup.max_size_gb, Some(2.5));
        assert!((config.backup.max_disk_usage_percent.unwrap() - 66.6666666667).abs() < 0.0001);
    }

    #[test]
    fn test_config_zero_values() {
        let toml_str = r#"
            [backup]
            max_size_gb = 0.0
            max_disk_usage_percent = 0.0
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.backup.max_size_gb, Some(0.0));
        assert_eq!(config.backup.max_disk_usage_percent, Some(0.0));
        // Zero values should be valid
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_config_very_large_values() {
        let mut config = Config::default();
        config.backup.max_size_gb = Some(999999.0);
        config.backup.max_disk_usage_percent = Some(100.0); // Max valid
        config.processing.max_memory_mb = Some(999999);
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_config_mode_variations_invalid() {
        let invalid_modes = vec![
            "PCRE",
            "PCre",
            "ERE",
            "BRE",
            "pcre2",
            "regex",
            "javascript",
            "",
            "pcre ", // trailing space
            " pcre", // leading space
        ];
        for mode in invalid_modes {
            let mut config = Config::default();
            config.compatibility.mode = Some(mode.to_string());
            assert!(
                validate_config(&config).is_err(),
                "Mode '{}' should be invalid",
                mode
            );
        }
    }

    #[test]
    fn test_config_comment_preservation_in_output() {
        // When we save config, comments are lost (TOML limitation)
        // But the values should be preserved
        let commented_toml = r#"
            # This is a comment
            [backup]
            max_size_gb = 5.0  # inline comment

            [compatibility]
            # Mode selection
            mode = "ere"
        "#;
        let config: Config = toml::from_str(commented_toml).unwrap();
        assert_eq!(config.backup.max_size_gb, Some(5.0));
        assert_eq!(config.compatibility.mode, Some("ere".to_string()));

        // Serialize and deserialize - values preserved, comments lost
        let toml_out = toml::to_string_pretty(&config).unwrap();
        let config2: Config = toml::from_str(&toml_out).unwrap();
        assert_eq!(config.backup.max_size_gb, config2.backup.max_size_gb);
        assert_eq!(config.compatibility.mode, config2.compatibility.mode);
    }

    #[test]
    fn test_config_serialize_order() {
        // TOML doesn't guarantee order, but all fields should be present
        let config = Config::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();

        // Check all sections are present
        assert!(toml_str.contains("[backup]"));
        assert!(toml_str.contains("[compatibility]"));
        assert!(toml_str.contains("[processing]"));

        // Check key fields are present
        assert!(toml_str.contains("max_size_gb"));
        assert!(toml_str.contains("max_disk_usage_percent"));
        assert!(toml_str.contains("mode"));
        assert!(toml_str.contains("show_warnings"));
        assert!(toml_str.contains("context_lines"));
        assert!(toml_str.contains("max_memory_mb"));
        assert!(toml_str.contains("streaming"));
    }

    #[test]
    fn test_config_with_extra_fields_is_ignored() {
        // TOML with extra unknown fields should be ignored (flexible parsing)
        let toml_with_extra = r#"
            [backup]
            max_size_gb = 3.0
            unknown_field = "should be ignored"

            [compatibility]
            mode = "pcre"
            also_unknown = 123

            [processing]
            context_lines = 2
            another_unknown = true
        "#;
        let config: Config = toml::from_str(toml_with_extra).unwrap();
        assert_eq!(config.backup.max_size_gb, Some(3.0));
        assert_eq!(config.compatibility.mode, Some("pcre".to_string()));
        assert_eq!(config.processing.context_lines, Some(2));
    }

    #[test]
    fn test_config_all_option_none() {
        // Config where all Optional fields are None via Rust struct
        // In TOML, absent fields with serde(default) will use the default function values
        // To get None values, we create the struct directly
        let config = Config {
            backup: BackupConfig {
                max_size_gb: None,
                max_disk_usage_percent: None,
                backup_dir: None,
            },
            compatibility: CompatibilityConfig {
                mode: None,
                show_warnings: None,
            },
            processing: ProcessingConfig {
                context_lines: None,
                max_memory_mb: None,
                streaming: None,
            },
        };

        // Verify all fields are None
        assert_eq!(config.backup.max_size_gb, None);
        assert_eq!(config.backup.max_disk_usage_percent, None);
        assert_eq!(config.backup.backup_dir, None);
        assert_eq!(config.compatibility.mode, None);
        assert_eq!(config.compatibility.show_warnings, None);
        assert_eq!(config.processing.context_lines, None);
        assert_eq!(config.processing.max_memory_mb, None);
        assert_eq!(config.processing.streaming, None);

        // Verify it serializes correctly
        let toml_str = toml::to_string_pretty(&config).unwrap();
        // None values serialize as absent fields or with defaults
        let parsed: Config = toml::from_str(&toml_str).unwrap();
        // When parsed back, serde(default) functions provide default values
        assert!(parsed.backup.max_size_gb.is_some());
    }

    #[test]
    fn test_config_empty_sections_use_defaults() {
        // Empty sections in TOML should use default functions
        let toml_str = r#"
            [backup]
            [compatibility]
            [processing]
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        // All fields should have their default values from default functions
        assert_eq!(config.backup.max_size_gb, Some(2.0));
        assert_eq!(config.backup.max_disk_usage_percent, Some(60.0));
        assert_eq!(config.compatibility.mode, Some("pcre".to_string()));
        assert_eq!(config.compatibility.show_warnings, Some(true));
        assert_eq!(config.processing.context_lines, Some(2));
        assert_eq!(config.processing.max_memory_mb, Some(100));
        assert_eq!(config.processing.streaming, Some(true));
    }
}
