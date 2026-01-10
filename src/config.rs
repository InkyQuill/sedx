/// Configuration management for SedX
///
/// SedX stores configuration in ~/.sedx/config.toml

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

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
fn default_max_size_gb() -> Option<f64> { Some(2.0) }
fn default_max_disk_usage_percent() -> Option<f64> { Some(60.0) }
fn default_mode() -> Option<String> { Some("pcre".to_string()) }
fn default_show_warnings() -> Option<bool> { Some(true) }
fn default_context_lines() -> Option<usize> { Some(2) }
fn default_max_memory_mb() -> Option<usize> { Some(100) }
fn default_streaming() -> Option<bool> { Some(true) }

/// Get the configuration file path
pub fn config_file_path() -> Result<PathBuf> {
    let home_dir = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;

    let config_dir = home_dir.join(".sedx");
    fs::create_dir_all(&config_dir)
        .with_context(|| format!("Failed to create config directory: {}", config_dir.display()))?;

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

    fs::write(&config_path, get_default_config_content())
        .with_context(|| format!("Failed to write default config file: {}", config_path.display()))?;

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
pub fn save_config(config: &Config) -> Result<()> {
    let config_path = config_file_path()?;

    let config_str = toml::to_string_pretty(config)
        .context("Failed to serialize config")?;

    fs::write(&config_path, config_str)
        .with_context(|| format!("Failed to write config file: {}", config_path.display()))?;

    Ok(())
}

/// Validate configuration values
pub fn validate_config(config: &Config) -> Result<()> {
    // Validate backup settings
    if let Some(max_gb) = config.backup.max_size_gb {
        if max_gb < 0.0 {
            anyhow::bail!("Invalid max_size_gb: {} (must be positive)", max_gb);
        }
    }

    if let Some(max_percent) = config.backup.max_disk_usage_percent {
        if max_percent < 0.0 || max_percent > 100.0 {
            anyhow::bail!("Invalid max_disk_usage_percent: {} (must be 0-100)", max_percent);
        }
    }

    // Validate compatibility mode
    if let Some(mode) = &config.compatibility.mode {
        if !["pcre", "ere", "bre"].contains(&mode.as_str()) {
            anyhow::bail!("Invalid mode: {} (must be 'pcre', 'ere', or 'bre')", mode);
        }
    }

    // Validate processing settings
    if let Some(context) = config.processing.context_lines {
        if context > 10 {
            anyhow::bail!("Invalid context_lines: {} (max 10)", context);
        }
    }

    if let Some(max_mb) = config.processing.max_memory_mb {
        if max_mb < 10 {
            anyhow::bail!("Invalid max_memory_mb: {} (min 10 MB)", max_mb);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_validate_config_valid() {
        let config = Config::default();
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_validate_config_invalid_max_size_gb() {
        let mut config = Config::default();
        config.backup.max_size_gb = Some(-1.0);
        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn test_validate_config_invalid_mode() {
        let mut config = Config::default();
        config.compatibility.mode = Some("invalid".to_string());
        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn test_config_to_toml() {
        let config = Config::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        assert!(toml_str.contains("[backup]"));
        assert!(toml_str.contains("[compatibility]"));
        assert!(toml_str.contains("[processing]"));
    }
}
