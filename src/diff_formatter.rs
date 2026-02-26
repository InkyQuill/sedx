use crate::file_processor::{ChangeType, FileChange, FileDiff};
use colored::*;
use std::io::IsTerminal;

pub struct DiffFormatter;

impl DiffFormatter {
    /// Auto-detect if we should use colors
    fn should_use_color() -> bool {
        // Check NO_COLOR env var (https://no-color.org/)
        if std::env::var("NO_COLOR").is_ok() {
            return false;
        }

        // Check if terminal supports color (Rust 1.70+)
        std::io::stdout().is_terminal()
    }

    /// Format file diff with context and new indicators
    pub fn format_diff_with_context(
        diff: &FileDiff,
        context_size: usize,
        _expression: &str,
    ) -> String {
        let use_color = Self::should_use_color();
        let mut output = String::new();

        // If there are printed lines, show only those (print command mode)
        if !diff.printed_lines.is_empty() {
            if use_color {
                output.push_str(&format!("{}\n", diff.file_path.bold().cyan()));
            } else {
                output.push_str(&format!("{}\n", diff.file_path));
            }

            for line in &diff.printed_lines {
                if use_color {
                    output.push_str(&format!("{}\n", line.white()));
                } else {
                    output.push_str(&format!("{}\n", line));
                }
            }

            return output;
        }

        // Regular diff mode
        if use_color {
            output.push_str(&format!("{}\n", diff.file_path.bold().cyan()));
        } else {
            output.push_str(&format!("{}\n", diff.file_path));
        }

        // Check if this is streaming mode (all_lines is empty)
        let lines_to_show = if diff.is_streaming && diff.all_lines.is_empty() {
            // Streaming mode: use changes directly without context
            Self::format_changes_streaming(&diff.changes, context_size)
        } else {
            // In-memory mode: use all_lines with context
            Self::filter_lines_with_context(&diff.all_lines, context_size)
        };

        for (line_num, content, change_type) in lines_to_show {
            // Special handling for "..." placeholder
            if content == "..." {
                if use_color {
                    output.push_str(&format!("{}\n", "...".dimmed()));
                } else {
                    output.push_str("...\n");
                }
                continue;
            }

            let indicator = match change_type {
                ChangeType::Unchanged => "=",
                ChangeType::Modified => "~",
                ChangeType::Added => "+",
                ChangeType::Deleted => "-",
            };

            if use_color {
                let colored_line = match change_type {
                    ChangeType::Unchanged => format!(
                        "L{}: {} {}\n",
                        line_num,
                        indicator.dimmed(),
                        content.dimmed()
                    ),
                    ChangeType::Modified => format!(
                        "L{}: {} {}\n",
                        line_num,
                        indicator.yellow().bold(),
                        content.yellow().bold()
                    ),
                    ChangeType::Added => format!(
                        "L{}: {} {}\n",
                        line_num,
                        indicator.green().bold(),
                        content.green().bold()
                    ),
                    ChangeType::Deleted => format!(
                        "L{}: {} {}\n",
                        line_num,
                        indicator.red().bold(),
                        content.red()
                    ),
                };
                output.push_str(&colored_line);
            } else {
                output.push_str(&format!("L{}: {} {}\n", line_num, indicator, content));
            }
        }

        // Summary
        let modified_count = diff
            .changes
            .iter()
            .filter(|c| c.change_type == ChangeType::Modified)
            .count();
        let added_count = diff
            .changes
            .iter()
            .filter(|c| c.change_type == ChangeType::Added)
            .count();
        let deleted_count = diff
            .changes
            .iter()
            .filter(|c| c.change_type == ChangeType::Deleted)
            .count();
        let total = modified_count + added_count + deleted_count;

        if use_color {
            output.push_str(&format!(
                "\nTotal: {} change",
                total.to_string().bold().white()
            ));
            if total != 1 {
                output.push('s');
            }
            let mut parts = Vec::new();
            if modified_count > 0 {
                parts.push(format!("{} {}", modified_count, "modified".yellow()));
            }
            if added_count > 0 {
                parts.push(format!("{} {}", added_count, "added".green()));
            }
            if deleted_count > 0 {
                parts.push(format!("{} {}", deleted_count, "deleted".red()));
            }
            if !parts.is_empty() {
                output.push_str(&format!(" ({})", parts.join(", ")));
            }
            output.push('\n');
        } else {
            output.push_str(&format!("\nTotal: {} changes", total));
            if modified_count > 0 || added_count > 0 || deleted_count > 0 {
                output.push_str(&format!(
                    " ({} modified, {} added, {} deleted)",
                    modified_count, added_count, deleted_count
                ));
            }
            output.push('\n');
        }

        output
    }

    /// Filter lines to show only changed lines with context, grouping close changes
    fn filter_lines_with_context(
        lines: &[(usize, String, ChangeType)],
        context_size: usize,
    ) -> Vec<(usize, String, ChangeType)> {
        if context_size == 0 {
            // Show only changed lines
            return lines
                .iter()
                .filter(|(_, _, ct)| *ct != ChangeType::Unchanged)
                .cloned()
                .collect();
        }

        // Find indices of all changed lines
        let changed_indices: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter(|(_, (_, _, ct))| *ct != ChangeType::Unchanged)
            .map(|(i, _)| i)
            .collect();

        if changed_indices.is_empty() {
            return Vec::new();
        }

        // Group changes that are close to each other
        // Two changes are in the same group if they're within (context_size * 2 + 1) lines
        let group_threshold = context_size * 2 + 1;
        let mut groups: Vec<Vec<usize>> = vec![vec![changed_indices[0]]];

        for &idx in &changed_indices[1..] {
            // SAFETY: groups is always non-empty because we return early above if changed_indices is empty,
            // and we initialize groups with vec![changed_indices[0]].
            // Each group is also always non-empty because we only push non-empty vectors.
            let last_group = groups.last_mut().unwrap();
            let last_idx = *last_group.last().unwrap();

            // If this change is close to the last change in the group, add to the same group
            if idx.saturating_sub(last_idx) <= group_threshold {
                last_group.push(idx);
            } else {
                // Otherwise start a new group
                groups.push(vec![idx]);
            }
        }

        // Build the result by including context around each group
        let mut result = Vec::new();
        let mut last_included_end = None;

        for group in groups.iter() {
            // SAFETY: Each group is always non-empty (created as vec![idx] or via push)
            let group_start = *group.first().unwrap();
            let group_end = *group.last().unwrap();

            // Calculate the range to include (with context)
            let start = group_start.saturating_sub(context_size);
            let end = (group_end + context_size + 1).min(lines.len());

            // Add "..." between distant groups (but not before the first group)
            if let Some(last_end) = last_included_end {
                // If there's a gap between groups and it's more than context_size lines
                if start > last_end + context_size {
                    // Insert a placeholder for "..."
                    result.push((0, "...".to_string(), ChangeType::Unchanged));
                }
            }

            // Add all lines in this group's range
            for i in start..end {
                if let Some(line) = lines.get(i) {
                    result.push(line.clone());
                }
            }

            last_included_end = Some(end);
        }

        result
    }

    /// Format changes in streaming mode (without storing all lines)
    /// For Chunk 6: Simple diff showing only changed lines without context
    fn format_changes_streaming(
        changes: &[crate::file_processor::LineChange],
        _context_size: usize,
    ) -> Vec<(usize, String, ChangeType)> {
        // In streaming mode (Chunk 6), we show only changed lines without context
        // This saves memory for large files
        changes
            .iter()
            .map(|c| (c.line_number, c.content.clone(), c.change_type.clone()))
            .collect()
    }

    /// Legacy method - format simple preview (backward compatibility)
    #[allow(dead_code)] // Kept for API compatibility
    pub fn format_preview(
        expression: &str,
        files_changes: Vec<(String, Vec<FileChange>)>,
    ) -> String {
        let use_color = Self::should_use_color();
        let mut output = String::new();

        if use_color {
            output.push_str(&format!(
                "{} {}\n\n",
                "🔍 Preview:".bold().green(),
                expression.white().bold()
            ));
        } else {
            output.push_str(&format!("Preview: {}\n\n", expression));
        }

        let total_changes: usize = files_changes.iter().map(|(_, changes)| changes.len()).sum();
        let file_count = files_changes.iter().filter(|(_, c)| !c.is_empty()).count();

        if total_changes == 0 {
            output.push_str("No changes would be made.\n");
            return output;
        }

        for (file, changes) in &files_changes {
            if changes.is_empty() {
                continue;
            }

            if use_color {
                output.push_str(&format!("{}\n", file.bold().cyan()));
            } else {
                output.push_str(&format!("{}\n", file));
            }

            for change in changes {
                output.push_str(&Self::format_legacy_change(change, use_color));
            }

            output.push('\n');
        }

        if use_color {
            output.push_str(&format!(
                "Total: {} changes across {} file{}\n\n",
                total_changes.to_string().bold().white(),
                file_count,
                if file_count == 1 { "" } else { "s" }
            ));

            output.push_str(&"Apply with: ".white());
            output.push_str(&format!("sedx '{}'\n", expression).bold().yellow());
        } else {
            output.push_str(&format!(
                "Total: {} changes across {} file{}\n\n",
                total_changes,
                file_count,
                if file_count == 1 { "" } else { "s" }
            ));
            output.push_str(&format!("Apply with: sedx '{}'\n", expression));
        }

        output
    }

    #[allow(dead_code)] // Used by format_preview
    fn format_legacy_change(change: &FileChange, use_color: bool) -> String {
        if use_color {
            format!(
                "  Line {}: {} {}\n  Line {}: {} {}\n",
                change.line_number.to_string().white().bold(),
                "-".red().bold(),
                change.old_content.red(),
                change.line_number.to_string().white().bold(),
                "+".green().bold(),
                change.new_content.green()
            )
        } else {
            format!(
                "  Line {}: - {}\n  Line {}: + {}\n",
                change.line_number, change.old_content, change.line_number, change.new_content
            )
        }
    }

    /// Format execute result with backup ID
    #[allow(dead_code)] // Kept for API compatibility
    pub fn format_execute_result(
        expression: &str,
        backup_id: &str,
        files_changes: Vec<(String, Vec<FileChange>)>,
    ) -> String {
        let use_color = Self::should_use_color();
        let mut output = String::new();

        if use_color {
            output.push_str(&format!(
                "{} {}\n",
                "✅ Applied:".bold().green(),
                expression.white().bold()
            ));
            output.push_str(&format!(
                "{} {}\n\n",
                "Backup ID:".white(),
                backup_id.yellow().bold()
            ));
        } else {
            output.push_str(&format!("Applied: {}\n", expression));
            output.push_str(&format!("Backup ID: {}\n\n", backup_id));
        }

        let total_changes: usize = files_changes.iter().map(|(_, changes)| changes.len()).sum();

        if total_changes > 0 {
            if use_color {
                output.push_str(&"Changes made:\n".bold().white());
            } else {
                output.push_str("Changes made:\n");
            }

            for (file, changes) in &files_changes {
                if changes.is_empty() {
                    continue;
                }
                if use_color {
                    output.push_str(&format!("  {}: {} changes\n", file.cyan(), changes.len()));
                } else {
                    output.push_str(&format!("  {}: {} changes\n", file, changes.len()));
                }
            }

            output.push('\n');
        }

        if use_color {
            output.push_str(&"Rollback with: ".white());
            output.push_str(&format!("sedx rollback {}\n", backup_id).bold().yellow());
        } else {
            output.push_str(&format!("Rollback with: sedx rollback {}\n", backup_id));
        }

        output
    }

    /// Format operation history
    pub fn format_history(backups: Vec<crate::backup_manager::BackupMetadata>) -> String {
        let use_color = Self::should_use_color();
        let mut output = String::new();

        if backups.is_empty() {
            output.push_str("No backup history found.\n");
            return output;
        }

        if use_color {
            output.push_str(&"Operation History:\n\n".bold().white());
        } else {
            output.push_str("Operation History:\n\n");
        }

        for backup in backups {
            if use_color {
                output.push_str(&format!("ID: {}\n", backup.id.yellow()));
                output.push_str(&format!(
                    "  Time: {}\n",
                    backup.timestamp.format("%Y-%m-%d %H:%M:%S")
                ));
                output.push_str(&format!("  Command: {}\n", backup.expression.cyan()));
                output.push_str(&format!("  Files: {}\n", backup.files.len()));
            } else {
                output.push_str(&format!("ID: {}\n", backup.id));
                output.push_str(&format!(
                    "  Time: {}\n",
                    backup.timestamp.format("%Y-%m-%d %H:%M:%S")
                ));
                output.push_str(&format!("  Command: {}\n", backup.expression));
                output.push_str(&format!("  Files: {}\n", backup.files.len()));
            }
            output.push('\n');
        }

        output
    }

    /// Format dry run header
    pub fn format_dry_run_header(expression: &str) -> String {
        let use_color = Self::should_use_color();

        if use_color {
            format!(
                "{} {}\n\n",
                "🔍 Dry run:".bold().cyan(),
                expression.white().bold()
            )
        } else {
            format!("Dry run: {}\n\n", expression)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup_manager::{BackupMetadata, FileBackup};
    use crate::file_processor::{ChangeType, FileChange, FileDiff, LineChange};
    use chrono::Utc;
    use std::path::PathBuf;

    // Helper function to create a test backup metadata
    fn create_test_backup(id: &str, expression: &str, files: Vec<&str>) -> BackupMetadata {
        BackupMetadata {
            id: id.to_string(),
            timestamp: Utc::now(),
            expression: expression.to_string(),
            files: files
                .into_iter()
                .map(|f| FileBackup {
                    original_path: PathBuf::from(f),
                    backup_path: PathBuf::from(format!("/tmp/backup/{}", f)),
                })
                .collect(),
        }
    }

    // Helper function to create a test file diff
    fn create_test_diff(
        file_path: &str,
        all_lines: Vec<(usize, String, ChangeType)>,
        changes: Vec<LineChange>,
    ) -> FileDiff {
        FileDiff {
            file_path: file_path.to_string(),
            changes,
            all_lines,
            printed_lines: Vec::new(),
            is_streaming: false,
        }
    }

    // Helper function to create a test line change
    fn create_test_line_change(
        line_number: usize,
        content: &str,
        change_type: ChangeType,
    ) -> LineChange {
        LineChange {
            line_number,
            change_type,
            content: content.to_string(),
            old_content: None,
        }
    }

    #[test]
    fn test_format_diff_with_context_single_change() {
        let all_lines = vec![
            (1, "line 1".to_string(), ChangeType::Unchanged),
            (2, "modified line".to_string(), ChangeType::Modified),
            (3, "line 3".to_string(), ChangeType::Unchanged),
        ];
        let changes = vec![create_test_line_change(
            2,
            "modified line",
            ChangeType::Modified,
        )];
        let diff = create_test_diff("test.txt", all_lines, changes);

        let result = DiffFormatter::format_diff_with_context(&diff, 0, "s/old/new/");

        // Should contain the file path
        assert!(result.contains("test.txt"));
        // Should contain the modified line
        assert!(result.contains("modified line"));
        // Should contain the total changes summary
        assert!(result.contains("Total:"));
        // Should indicate 1 modified
        assert!(result.contains("1 modified"));
    }

    #[test]
    fn test_format_diff_with_context_multiple_changes() {
        let all_lines = vec![
            (1, "line 1".to_string(), ChangeType::Unchanged),
            (2, "modified line 1".to_string(), ChangeType::Modified),
            (3, "line 3".to_string(), ChangeType::Unchanged),
            (4, "added line".to_string(), ChangeType::Added),
            (5, "line 5".to_string(), ChangeType::Unchanged),
            (6, "deleted line".to_string(), ChangeType::Deleted),
        ];
        let changes = vec![
            create_test_line_change(2, "modified line 1", ChangeType::Modified),
            create_test_line_change(4, "added line", ChangeType::Added),
            create_test_line_change(6, "deleted line", ChangeType::Deleted),
        ];
        let diff = create_test_diff("test.txt", all_lines, changes);

        let result = DiffFormatter::format_diff_with_context(&diff, 0, "s/old/new/");

        // Should contain all change types
        assert!(result.contains("modified"));
        assert!(result.contains("added"));
        assert!(result.contains("deleted"));
        // Should indicate 3 total changes
        assert!(result.contains("Total: 3 changes"));
    }

    #[test]
    fn test_format_diff_with_context_with_context_lines() {
        let all_lines = vec![
            (1, "context before".to_string(), ChangeType::Unchanged),
            (2, "context before 2".to_string(), ChangeType::Unchanged),
            (3, "modified line".to_string(), ChangeType::Modified),
            (4, "context after".to_string(), ChangeType::Unchanged),
            (5, "context after 2".to_string(), ChangeType::Unchanged),
        ];
        let changes = vec![create_test_line_change(
            3,
            "modified line",
            ChangeType::Modified,
        )];
        let diff = create_test_diff("test.txt", all_lines, changes);

        let result = DiffFormatter::format_diff_with_context(&diff, 2, "s/old/new/");

        // Should include context lines
        assert!(result.contains("context before"));
        assert!(result.contains("context after"));
        // Should include the modified line
        assert!(result.contains("modified line"));
    }

    #[test]
    fn test_format_diff_with_context_empty_changes() {
        let all_lines = vec![
            (1, "line 1".to_string(), ChangeType::Unchanged),
            (2, "line 2".to_string(), ChangeType::Unchanged),
        ];
        let diff = create_test_diff("test.txt", all_lines, vec![]);

        let result = DiffFormatter::format_diff_with_context(&diff, 0, "s/old/new/");

        // Should contain the file path
        assert!(result.contains("test.txt"));
        // Should indicate 0 total changes
        assert!(result.contains("Total: 0 changes"));
    }

    #[test]
    fn test_format_diff_with_context_distant_changes() {
        let all_lines = vec![
            (1, "line 1".to_string(), ChangeType::Unchanged),
            (2, "modified 1".to_string(), ChangeType::Modified),
            (3, "line 3".to_string(), ChangeType::Unchanged),
            (4, "line 4".to_string(), ChangeType::Unchanged),
            (5, "line 5".to_string(), ChangeType::Unchanged),
            (6, "line 6".to_string(), ChangeType::Unchanged),
            (7, "modified 2".to_string(), ChangeType::Modified),
            (8, "line 8".to_string(), ChangeType::Unchanged),
        ];
        let changes = vec![
            create_test_line_change(2, "modified 1", ChangeType::Modified),
            create_test_line_change(7, "modified 2", ChangeType::Modified),
        ];
        let diff = create_test_diff("test.txt", all_lines, changes);

        let result = DiffFormatter::format_diff_with_context(&diff, 1, "s/old/new/");

        // Should contain "..." placeholder for distant groups
        assert!(result.contains("..."));
    }

    #[test]
    fn test_format_diff_with_context_streaming_mode() {
        let changes = vec![
            create_test_line_change(1, "modified line 1", ChangeType::Modified),
            create_test_line_change(5, "modified line 2", ChangeType::Modified),
        ];
        let diff = FileDiff {
            file_path: "test.txt".to_string(),
            changes,
            all_lines: vec![], // Empty for streaming mode
            printed_lines: vec![],
            is_streaming: true, // Streaming mode
        };

        let result = DiffFormatter::format_diff_with_context(&diff, 2, "s/old/new/");

        // Should still show changes in streaming mode
        assert!(result.contains("modified line 1"));
        assert!(result.contains("modified line 2"));
        assert!(result.contains("Total:"));
    }

    #[test]
    fn test_format_diff_with_context_printed_lines_mode() {
        let diff = FileDiff {
            file_path: "test.txt".to_string(),
            changes: vec![],
            all_lines: vec![],
            printed_lines: vec!["printed line 1".to_string(), "printed line 2".to_string()],
            is_streaming: false,
        };

        let result = DiffFormatter::format_diff_with_context(&diff, 0, "/pattern/p");

        // Should show printed lines
        assert!(result.contains("printed line 1"));
        assert!(result.contains("printed line 2"));
        // Should not show Total summary for printed lines mode
        assert!(!result.contains("Total:"));
    }

    #[test]
    fn test_format_dry_run_header_basic() {
        let result = DiffFormatter::format_dry_run_header("s/foo/bar/");

        assert!(result.contains("Dry run"));
        assert!(result.contains("s/foo/bar/"));
        assert!(result.ends_with("\n\n"));
    }

    #[test]
    fn test_format_dry_run_header_complex_expression() {
        let result = DiffFormatter::format_dry_run_header("1,10{s/foo/bar/; s/baz/qux/}");

        assert!(result.contains("Dry run"));
        assert!(result.contains("1,10{s/foo/bar/; s/baz/qux/}"));
    }

    #[test]
    fn test_format_dry_run_header_with_special_chars() {
        let result = DiffFormatter::format_dry_run_header("s/.*\n\t//g");

        assert!(result.contains("Dry run"));
        assert!(result.contains("s/.*\n\t//g"));
    }

    #[test]
    fn test_format_history_empty() {
        let result = DiffFormatter::format_history(vec![]);

        assert_eq!(result, "No backup history found.\n");
    }

    #[test]
    fn test_format_history_single_backup() {
        let backup = create_test_backup("backup-123", "s/foo/bar/", vec!["file1.txt", "file2.txt"]);
        let result = DiffFormatter::format_history(vec![backup]);

        assert!(result.contains("Operation History"));
        assert!(result.contains("backup-123"));
        assert!(result.contains("s/foo/bar/"));
        assert!(result.contains("Files: 2"));
    }

    #[test]
    fn test_format_history_multiple_backups() {
        let backup1 = create_test_backup("backup-001", "s/foo/bar/", vec!["file1.txt"]);
        let backup2 =
            create_test_backup("backup-002", "s/baz/qux/", vec!["file2.txt", "file3.txt"]);
        let result = DiffFormatter::format_history(vec![backup1, backup2]);

        assert!(result.contains("backup-001"));
        assert!(result.contains("s/foo/bar/"));
        assert!(result.contains("Files: 1"));
        assert!(result.contains("backup-002"));
        assert!(result.contains("s/baz/qux/"));
        assert!(result.contains("Files: 2"));
    }

    #[test]
    fn test_format_history_chronological_ordering() {
        // Create backups with different timestamps
        let mut backup1 = create_test_backup("backup-old", "s/old/new/", vec!["file1.txt"]);
        let mut backup2 = create_test_backup("backup-new", "s/new/old/", vec!["file2.txt"]);

        // Manually set timestamps to ensure ordering
        backup1.timestamp = Utc::now() - chrono::Duration::days(1);
        backup2.timestamp = Utc::now();

        let result = DiffFormatter::format_history(vec![backup1, backup2]);

        // Both backups should appear in the result
        assert!(result.contains("backup-old"));
        assert!(result.contains("backup-new"));
    }

    #[test]
    fn test_format_history_with_no_files() {
        let backup = BackupMetadata {
            id: "backup-empty".to_string(),
            timestamp: Utc::now(),
            expression: "s/nochange/nochange/".to_string(),
            files: vec![],
        };
        let result = DiffFormatter::format_history(vec![backup]);

        assert!(result.contains("backup-empty"));
        assert!(result.contains("Files: 0"));
    }

    #[test]
    fn test_format_execute_result_single_file() {
        let files_changes = vec![(
            "test.txt".to_string(),
            vec![FileChange {
                line_number: 1,
                old_content: "old".to_string(),
                new_content: "new".to_string(),
            }],
        )];
        let result =
            DiffFormatter::format_execute_result("s/old/new/", "backup-123", files_changes);

        assert!(result.contains("Applied"));
        assert!(result.contains("s/old/new/"));
        assert!(result.contains("backup-123"));
        assert!(result.contains("test.txt"));
        assert!(result.contains("Rollback with: sedx rollback backup-123"));
    }

    #[test]
    fn test_format_execute_result_multiple_files() {
        let files_changes = vec![
            (
                "file1.txt".to_string(),
                vec![FileChange {
                    line_number: 1,
                    old_content: "old".to_string(),
                    new_content: "new".to_string(),
                }],
            ),
            (
                "file2.txt".to_string(),
                vec![
                    FileChange {
                        line_number: 1,
                        old_content: "foo".to_string(),
                        new_content: "bar".to_string(),
                    },
                    FileChange {
                        line_number: 2,
                        old_content: "baz".to_string(),
                        new_content: "qux".to_string(),
                    },
                ],
            ),
        ];
        let result =
            DiffFormatter::format_execute_result("s/foo/bar/", "backup-456", files_changes);

        assert!(result.contains("file1.txt"));
        assert!(result.contains("1 changes"));
        assert!(result.contains("file2.txt"));
        assert!(result.contains("2 changes"));
        assert!(result.contains("Rollback with: sedx rollback backup-456"));
    }

    #[test]
    fn test_format_execute_result_no_changes() {
        let files_changes = vec![("test.txt".to_string(), vec![])];
        let result =
            DiffFormatter::format_execute_result("s/nochange/", "backup-789", files_changes);

        assert!(result.contains("Applied"));
        assert!(result.contains("backup-789"));
        assert!(result.contains("Rollback with:"));
        // Should not show "Changes made:" section
        assert!(!result.contains("Changes made:"));
    }

    #[test]
    fn test_format_preview_with_changes() {
        let files_changes = vec![(
            "test.txt".to_string(),
            vec![FileChange {
                line_number: 1,
                old_content: "old".to_string(),
                new_content: "new".to_string(),
            }],
        )];
        let result = DiffFormatter::format_preview("s/old/new/", files_changes);

        assert!(result.contains("Preview"));
        assert!(result.contains("s/old/new/"));
        assert!(result.contains("test.txt"));
        assert!(result.contains("old"));
        assert!(result.contains("new"));
        assert!(result.contains("Apply with: sedx"));
    }

    #[test]
    fn test_format_preview_no_changes() {
        let files_changes = vec![("test.txt".to_string(), vec![])];
        let result = DiffFormatter::format_preview("s/nochange/", files_changes);

        assert!(result.contains("No changes would be made"));
        assert!(!result.contains("Apply with:"));
    }

    #[test]
    fn test_preview_single_file_pluralization() {
        let files_changes = vec![(
            "test.txt".to_string(),
            vec![FileChange {
                line_number: 1,
                old_content: "old".to_string(),
                new_content: "new".to_string(),
            }],
        )];
        let result = DiffFormatter::format_preview("s/old/new/", files_changes);

        // Should say "1 file" (singular)
        assert!(result.contains("1 file"));
    }

    #[test]
    fn test_preview_multiple_files_pluralization() {
        let files_changes = vec![
            (
                "file1.txt".to_string(),
                vec![FileChange {
                    line_number: 1,
                    old_content: "old".to_string(),
                    new_content: "new".to_string(),
                }],
            ),
            (
                "file2.txt".to_string(),
                vec![FileChange {
                    line_number: 1,
                    old_content: "old".to_string(),
                    new_content: "new".to_string(),
                }],
            ),
        ];
        let result = DiffFormatter::format_preview("s/old/new/", files_changes);

        // Should say "2 files" (plural)
        assert!(result.contains("2 files"));
    }

    #[test]
    fn test_filter_lines_with_context_no_context() {
        let all_lines = vec![
            (1, "line 1".to_string(), ChangeType::Unchanged),
            (2, "modified".to_string(), ChangeType::Modified),
            (3, "line 3".to_string(), ChangeType::Unchanged),
        ];

        let result = DiffFormatter::filter_lines_with_context(&all_lines, 0);

        // Should only return changed lines
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, 2);
        assert_eq!(result[0].1, "modified");
        assert_eq!(result[0].2, ChangeType::Modified);
    }

    #[test]
    fn test_filter_lines_with_context_empty_input() {
        let all_lines = vec![];
        let result = DiffFormatter::filter_lines_with_context(&all_lines, 2);

        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_filter_lines_with_context_no_changes() {
        let all_lines = vec![
            (1, "line 1".to_string(), ChangeType::Unchanged),
            (2, "line 2".to_string(), ChangeType::Unchanged),
            (3, "line 3".to_string(), ChangeType::Unchanged),
        ];
        let result = DiffFormatter::filter_lines_with_context(&all_lines, 2);

        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_filter_lines_with_context_groups_nearby_changes() {
        let all_lines = vec![
            (1, "line 1".to_string(), ChangeType::Unchanged),
            (2, "modified 1".to_string(), ChangeType::Modified),
            (3, "line 3".to_string(), ChangeType::Unchanged),
            (4, "modified 2".to_string(), ChangeType::Modified),
            (5, "line 5".to_string(), ChangeType::Unchanged),
        ];
        let result = DiffFormatter::filter_lines_with_context(&all_lines, 1);

        // With context_size=1, threshold is 1*2+1=3
        // Changes at indices 1 and 3 are within threshold (3-1=2 <= 3)
        // So they should be in the same group with context
        assert!(result.len() > 2); // Should include both changes and context
    }

    #[test]
    fn test_filter_lines_with_context_adds_ellipsis_for_distant_groups() {
        let all_lines = vec![
            (1, "line 1".to_string(), ChangeType::Modified),
            (2, "line 2".to_string(), ChangeType::Unchanged),
            (3, "line 3".to_string(), ChangeType::Unchanged),
            (4, "line 4".to_string(), ChangeType::Unchanged),
            (5, "line 5".to_string(), ChangeType::Unchanged),
            (6, "line 6".to_string(), ChangeType::Modified),
        ];
        let result = DiffFormatter::filter_lines_with_context(&all_lines, 1);

        // Should contain "..." for distant groups
        let has_ellipsis = result.iter().any(|(_, content, _)| content == "...");
        assert!(
            has_ellipsis,
            "Expected '...' placeholder in result for distant groups"
        );
    }

    #[test]
    fn test_format_changes_streaming() {
        let changes = vec![
            LineChange {
                line_number: 1,
                content: "modified 1".to_string(),
                change_type: ChangeType::Modified,
                old_content: None,
            },
            LineChange {
                line_number: 5,
                content: "modified 2".to_string(),
                change_type: ChangeType::Added,
                old_content: None,
            },
        ];
        let result = DiffFormatter::format_changes_streaming(&changes, 2);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].0, 1);
        assert_eq!(result[0].1, "modified 1");
        assert_eq!(result[0].2, ChangeType::Modified);
        assert_eq!(result[1].0, 5);
        assert_eq!(result[1].1, "modified 2");
        assert_eq!(result[1].2, ChangeType::Added);
    }

    #[test]
    fn test_format_changes_streaming_empty() {
        let changes = vec![];
        let result = DiffFormatter::format_changes_streaming(&changes, 2);

        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_change_type_enum_unchanged() {
        let ct = ChangeType::Unchanged;
        // Ensure it can be cloned and compared
        let ct_clone = ct.clone();
        assert_eq!(ct, ct_clone);
    }

    #[test]
    fn test_change_type_enum_modified() {
        let ct = ChangeType::Modified;
        let ct_clone = ct.clone();
        assert_eq!(ct, ct_clone);
        assert_ne!(ct, ChangeType::Unchanged);
    }

    #[test]
    fn test_change_type_enum_added() {
        let ct = ChangeType::Added;
        assert_ne!(ct, ChangeType::Modified);
        assert_ne!(ct, ChangeType::Unchanged);
    }

    #[test]
    fn test_change_type_enum_deleted() {
        let ct = ChangeType::Deleted;
        assert_ne!(ct, ChangeType::Added);
        assert_ne!(ct, ChangeType::Modified);
        assert_ne!(ct, ChangeType::Unchanged);
    }

    #[test]
    fn test_line_change_creation() {
        let line_change = LineChange {
            line_number: 42,
            content: "test content".to_string(),
            change_type: ChangeType::Modified,
            old_content: Some("old content".to_string()),
        };

        assert_eq!(line_change.line_number, 42);
        assert_eq!(line_change.content, "test content");
        assert_eq!(line_change.change_type, ChangeType::Modified);
        assert_eq!(line_change.old_content, Some("old content".to_string()));
    }

    #[test]
    fn test_line_change_clone() {
        let line_change = LineChange {
            line_number: 1,
            content: "content".to_string(),
            change_type: ChangeType::Added,
            old_content: None,
        };
        let cloned = line_change.clone();

        assert_eq!(cloned.line_number, line_change.line_number);
        assert_eq!(cloned.content, line_change.content);
        assert_eq!(cloned.change_type, line_change.change_type);
    }

    #[test]
    fn test_file_diff_all_change_types() {
        // Test that all change types work in a FileDiff
        let all_lines = vec![
            (1, "unchanged".to_string(), ChangeType::Unchanged),
            (2, "modified".to_string(), ChangeType::Modified),
            (3, "added".to_string(), ChangeType::Added),
            (4, "deleted".to_string(), ChangeType::Deleted),
        ];
        let changes = vec![
            create_test_line_change(2, "modified", ChangeType::Modified),
            create_test_line_change(3, "added", ChangeType::Added),
            create_test_line_change(4, "deleted", ChangeType::Deleted),
        ];
        let diff = create_test_diff("test.txt", all_lines, changes);

        let result = DiffFormatter::format_diff_with_context(&diff, 0, "test/");

        // Verify all change types are represented
        assert!(result.contains("modified"));
        assert!(result.contains("added"));
        assert!(result.contains("deleted"));
        assert!(result.contains("Total: 3 changes"));
    }

    #[test]
    fn test_format_diff_indicators() {
        let all_lines = vec![
            (1, "unchanged".to_string(), ChangeType::Unchanged),
            (2, "modified".to_string(), ChangeType::Modified),
            (3, "added".to_string(), ChangeType::Added),
            (4, "deleted".to_string(), ChangeType::Deleted),
        ];
        let changes = vec![
            create_test_line_change(2, "modified", ChangeType::Modified),
            create_test_line_change(3, "added", ChangeType::Added),
            create_test_line_change(4, "deleted", ChangeType::Deleted),
        ];
        let diff = create_test_diff("test.txt", all_lines, changes);

        let result = DiffFormatter::format_diff_with_context(&diff, 0, "test/");

        // With context_size=0, unchanged lines are filtered out
        // Check for indicators on changed lines
        assert!(
            result.contains("L2:"),
            "Should contain L2 for modified line"
        );
        assert!(result.contains("L3:"), "Should contain L3 for added line");
        assert!(result.contains("L4:"), "Should contain L4 for deleted line");

        // Check for the content
        assert!(result.contains("modified"));
        assert!(result.contains("added"));
        assert!(result.contains("deleted"));
    }

    #[test]
    fn test_filter_lines_with_context_boundary_conditions() {
        // Test with changes at the beginning and end of file
        let all_lines = vec![
            (1, "first modified".to_string(), ChangeType::Modified),
            (2, "line 2".to_string(), ChangeType::Unchanged),
            (3, "line 3".to_string(), ChangeType::Unchanged),
            (4, "last modified".to_string(), ChangeType::Modified),
        ];
        let result = DiffFormatter::filter_lines_with_context(&all_lines, 1);

        // Should handle boundaries correctly
        assert!(!result.is_empty());
        // First result should be near line 1
        assert_eq!(result[0].0, 1);
        // Last result should be near line 4
        assert_eq!(result[result.len() - 1].0, 4);
    }

    #[test]
    fn test_filter_lines_with_context_single_change() {
        let all_lines = vec![
            (1, "line 1".to_string(), ChangeType::Unchanged),
            (2, "modified".to_string(), ChangeType::Modified),
            (3, "line 3".to_string(), ChangeType::Unchanged),
        ];
        let result = DiffFormatter::filter_lines_with_context(&all_lines, 1);

        // Should include the change and context
        assert!(!result.is_empty());
        let has_modified = result
            .iter()
            .any(|(_, content, ct)| content == "modified" && *ct == ChangeType::Modified);
        assert!(has_modified);
    }

    #[test]
    fn test_filter_lines_with_context_large_context_size() {
        let all_lines = vec![
            (1, "line 1".to_string(), ChangeType::Unchanged),
            (2, "line 2".to_string(), ChangeType::Unchanged),
            (3, "modified".to_string(), ChangeType::Modified),
            (4, "line 4".to_string(), ChangeType::Unchanged),
            (5, "line 5".to_string(), ChangeType::Unchanged),
        ];
        let result = DiffFormatter::filter_lines_with_context(&all_lines, 10);

        // With large context, should include most/all lines
        assert!(result.len() >= 3); // At minimum: the change and some context
    }

    #[test]
    fn test_format_history_various_expressions() {
        let backups = vec![
            create_test_backup("b1", "s/foo/bar/", vec!["f1.txt"]),
            create_test_backup("b2", "1,10d", vec!["f2.txt"]),
            create_test_backup("b3", "/pattern/p", vec!["f3.txt"]),
        ];
        let result = DiffFormatter::format_history(backups);

        assert!(result.contains("s/foo/bar/"));
        assert!(result.contains("1,10d"));
        assert!(result.contains("/pattern/p"));
    }
}
