//! SedX: Safe, modern replacement for GNU sed
//!
//! This library exposes SedX's core functionality for use in property-based tests.
//! The main binary is at src/main.rs.

pub mod backup_manager;
pub mod bre_converter;
pub mod capability;
pub mod cli;
pub mod command;
pub mod config;
pub mod diff_formatter;
pub mod disk_space;
pub mod ere_converter;
pub mod file_processor;
pub mod parser;
pub mod sed_parser;

// Re-export commonly used types for convenience
pub use backup_manager::{BackupManager, BackupMetadata, FileBackup};
pub use capability::can_stream;
pub use cli::RegexFlavor;
pub use command::{Address, Command, SubstitutionFlags};
pub use file_processor::{FileProcessor, StreamProcessor, LineChange, ChangeType};
pub use parser::Parser;
