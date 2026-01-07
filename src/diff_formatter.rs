use crate::file_processor::{FileDiff, ChangeType, FileChange};
use colored::*;

pub struct DiffFormatter;

impl DiffFormatter {
    /// Auto-detect if we should use colors
    fn should_use_color() -> bool {
        // Check NO_COLOR env var (https://no-color.org/)
        if std::env::var("NO_COLOR").is_ok() {
            return false;
        }

        // Check if terminal supports color
        atty::is(atty::Stream::Stdout)
    }

    /// Format file diff with context and new indicators
    pub fn format_diff_with_context(diff: &FileDiff, context_size: usize, _expression: &str) -> String {
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
                    ChangeType::Unchanged => format!("L{}: {} {}\n", line_num, indicator.dimmed(), content.dimmed()),
                    ChangeType::Modified => format!("L{}: {} {}\n", line_num, indicator.yellow().bold(), content.yellow().bold()),
                    ChangeType::Added => format!("L{}: {} {}\n", line_num, indicator.green().bold(), content.green().bold()),
                    ChangeType::Deleted => format!("L{}: {} {}\n", line_num, indicator.red().bold(), content.red()),
                };
                output.push_str(&colored_line);
            } else {
                output.push_str(&format!("L{}: {} {}\n", line_num, indicator, content));
            }
        }

        // Summary
        let modified_count = diff.changes.iter().filter(|c| c.change_type == ChangeType::Modified).count();
        let added_count = diff.changes.iter().filter(|c| c.change_type == ChangeType::Added).count();
        let deleted_count = diff.changes.iter().filter(|c| c.change_type == ChangeType::Deleted).count();
        let total = modified_count + added_count + deleted_count;

        if use_color {
            output.push_str(&format!("\nTotal: {} change", total.to_string().bold().white()));
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
                output.push_str(&format!(" ({} modified, {} added, {} deleted)", modified_count, added_count, deleted_count));
            }
            output.push('\n');
        }

        output
    }

    /// Filter lines to show only changed lines with context, grouping close changes
    fn filter_lines_with_context(
        lines: &[(usize, String, ChangeType)],
        context_size: usize
    ) -> Vec<(usize, String, ChangeType)> {
        if context_size == 0 {
            // Show only changed lines
            return lines.iter()
                .filter(|(_, _, ct)| *ct != ChangeType::Unchanged)
                .cloned()
                .collect();
        }

        // Find indices of all changed lines
        let changed_indices: Vec<usize> = lines.iter()
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

        for (_group_idx, group) in groups.iter().enumerate() {
            let group_start = group.first().unwrap();
            let group_end = group.last().unwrap();

            // Calculate the range to include (with context)
            let start = if *group_start >= context_size { *group_start - context_size } else { 0 };
            let end = (*group_end + context_size + 1).min(lines.len());

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
        _context_size: usize
    ) -> Vec<(usize, String, ChangeType)> {
        // In streaming mode (Chunk 6), we show only changed lines without context
        // This saves memory for large files
        changes.iter()
            .map(|c| (c.line_number, c.content.clone(), c.change_type.clone()))
            .collect()
    }

    /// Legacy method - format simple preview (backward compatibility)
    pub fn format_preview(expression: &str, files_changes: Vec<(String, Vec<FileChange>)>) -> String {
        let use_color = Self::should_use_color();
        let mut output = String::new();

        if use_color {
            output.push_str(&format!("{} {}\n\n", "🔍 Preview:".bold().green(), expression.white().bold()));
        } else {
            output.push_str(&format!("Preview: {}\n\n", expression));
        }

        let total_changes: usize = files_changes.iter().map(|(_, changes)| changes.len()).sum();
        let file_count = files_changes.iter().filter(|(_, c)| !c.is_empty()).count();

        if total_changes == 0 {
            if use_color {
                output.push_str("No changes would be made.\n");
            } else {
                output.push_str("No changes would be made.\n");
            }
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
            output.push_str(&format!("Total: {} changes across {} file{}\n\n",
                total_changes.to_string().bold().white(),
                file_count,
                if file_count == 1 { "" } else { "s" }
            ));

            output.push_str(&"Apply with: ".white());
            output.push_str(&format!("sedx '{}'\n", expression).bold().yellow());
        } else {
            output.push_str(&format!("Total: {} changes across {} file{}\n\n",
                total_changes,
                file_count,
                if file_count == 1 { "" } else { "s" }
            ));
            output.push_str(&format!("Apply with: sedx '{}'\n", expression));
        }

        output
    }

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
                change.line_number,
                change.old_content,
                change.line_number,
                change.new_content
            )
        }
    }

    /// Format execute result with backup ID
    pub fn format_execute_result(expression: &str, backup_id: &str, files_changes: Vec<(String, Vec<FileChange>)>) -> String {
        let use_color = Self::should_use_color();
        let mut output = String::new();

        if use_color {
            output.push_str(&format!("{} {}\n", "✅ Applied:".bold().green(), expression.white().bold()));
            output.push_str(&format!("{} {}\n\n", "Backup ID:".white(), backup_id.yellow().bold()));
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
            if use_color {
                output.push_str("No backup history found.\n");
            } else {
                output.push_str("No backup history found.\n");
            }
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
                output.push_str(&format!("  Time: {}\n", backup.timestamp.format("%Y-%m-%d %H:%M:%S")));
                output.push_str(&format!("  Command: {}\n", backup.expression.cyan()));
                output.push_str(&format!("  Files: {}\n", backup.files.len()));
            } else {
                output.push_str(&format!("ID: {}\n", backup.id));
                output.push_str(&format!("  Time: {}\n", backup.timestamp.format("%Y-%m-%d %H:%M:%S")));
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
            format!("{} {}\n\n", "🔍 Dry run:".bold().cyan(), expression.white().bold())
        } else {
            format!("Dry run: {}\n\n", expression)
        }
    }
}
