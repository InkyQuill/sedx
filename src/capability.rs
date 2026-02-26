//! Streaming Capability Checks
//!
//! This module provides functions to determine whether commands can be executed
//! in streaming mode or require full file buffering.

use crate::command::{Address, Command};

/// Check if a list of commands can be executed in streaming mode
///
/// # Streaming Limitations
///
/// Some commands require full file buffering and cannot run in streaming mode:
/// - Command groups with ranges
/// - Hold space operations with non-streamable ranges (e.g., negated addresses)
/// - Negated addresses in ranges
/// - Complex mixed ranges (pattern to negated pattern, etc.)
#[allow(dead_code)] // Kept for potential future use
pub fn can_stream(commands: &[Command]) -> bool {
    for cmd in commands {
        match cmd {
            Command::Substitution { range, .. } => {
                if let Some(range) = range
                    && !is_range_streamable(range)
                {
                    return false;
                }
            }
            Command::Delete { range } | Command::Print { range } => {
                if !is_range_streamable(range) {
                    return false;
                }
            }
            Command::Insert { .. } | Command::Append { .. } | Command::Change { .. } => {
                // Insert/Append/Change are streamable for single-line addresses
                // but not for ranges
                return true;
            }
            Command::Group {
                range,
                commands: inner_cmds,
            } => {
                // Chunk 10: Groups are streamable if range is streamable and inner commands are streamable
                if let Some(r) = range
                    && !is_range_streamable(r)
                {
                    return false;
                }
                // Check inner commands
                if !can_stream(inner_cmds) {
                    return false;
                }
            }
            Command::Hold { range }
            | Command::HoldAppend { range }
            | Command::Get { range }
            | Command::GetAppend { range }
            | Command::Exchange { range } => {
                // Chunk 9: Hold space operations are streamable
                // Check if range is streamable
                if let Some(r) = range
                    && !is_range_streamable(r)
                {
                    return false;
                }
            }
            Command::Quit { .. } => {
                // Quit is streamable
                continue;
            }
            Command::QuitWithoutPrint { .. } => {
                // Quit without printing is streamable
                continue;
            }
            // Phase 4: Multi-line pattern space commands are NOT streamable (require full file access)
            Command::Next { .. }
            | Command::NextAppend { .. }
            | Command::PrintFirstLine { .. }
            | Command::DeleteFirstLine { .. } => {
                return false;
            }
            // Phase 5: Flow control commands are NOT streamable (require label tracking and program counter)
            Command::Label { .. }
            | Command::Branch { .. }
            | Command::Test { .. }
            | Command::TestFalse { .. } => {
                return false;
            }
            // Phase 5: File I/O commands are NOT streamable (require file handle management)
            Command::ReadFile { .. }
            | Command::WriteFile { .. }
            | Command::ReadLine { .. }
            | Command::WriteFirstLine { .. } => {
                return false;
            }
            // Phase 5: Additional commands (print line number, print filename, clear pattern space)
            // PrintLineNumber and PrintFilename write to stdout separately
            // ClearPatternSpace modifies pattern space state
            Command::PrintLineNumber { .. }
            | Command::PrintFilename { .. }
            | Command::ClearPatternSpace { .. } => {
                return false;
            }
        }
    }
    true
}

/// Check if a specific address range type is supported in streaming mode
///
/// # Streamable Ranges
///
/// - Line number to line number: `1,10`
/// - First to last: `1,$`
/// - Pattern to pattern: `/start/,/end/`
/// - Pattern to line number: `/start/,10`
/// - Line number to pattern: `5,/end/`
/// - Pattern with relative offset: `/start/,+5`
/// - Stepping addresses: `1~2`
///
/// # Non-Streamable Ranges
///
/// - Negated addresses: `!/pattern/`
/// - Complex mixed negated ranges
#[allow(dead_code)] // Used by can_stream
fn is_range_streamable(range: &(Address, Address)) -> bool {
    use Address::*;

    match (&range.0, &range.1) {
        // Line number to line number - streamable
        (LineNumber(_), LineNumber(_)) => true,

        // First to last - streamable
        (LineNumber(1), LastLine) => true,

        // Pattern to pattern - streamable (uses state machine)
        (Pattern(_), Pattern(_)) => true,

        // Pattern to line number - streamable (mixed)
        (Pattern(_), LineNumber(_)) => true,

        // Line number to pattern - streamable (mixed)
        (LineNumber(_), Pattern(_)) => true,

        // Pattern to relative offset - streamable
        (Pattern(_), Relative { .. }) => true,

        // Line number to relative offset - streamable
        (LineNumber(_), Relative { .. }) => true,

        // Stepping addresses - streamable
        (Step { .. }, _) | (_, Step { .. }) => true,

        // Negated addresses - not streamable
        (Negated(_), _) | (_, Negated(_)) => false,

        // Relative offsets as start address - not streamable
        (Relative { .. }, _) => false,

        // First line as start - streamable with most end addresses
        (FirstLine, LineNumber(_)) => true,
        (FirstLine, LastLine) => true,
        (FirstLine, Pattern(_)) => true,

        // Last line as start - not streamable (need to know where end is)
        (LastLine, _) => false,

        // Default: conservative - not streamable
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::SubstitutionFlags;

    #[test]
    fn test_can_stream_simple_substitution() {
        let cmd = Command::Substitution {
            pattern: "foo".to_string(),
            replacement: "bar".to_string(),
            flags: SubstitutionFlags::default(),
            range: None,
        };
        assert!(can_stream(&[cmd]));
    }

    #[test]
    fn test_can_stream_substitution_with_line_range() {
        let cmd = Command::Substitution {
            pattern: "foo".to_string(),
            replacement: "bar".to_string(),
            flags: SubstitutionFlags::default(),
            range: Some((Address::LineNumber(1), Address::LineNumber(10))),
        };
        assert!(can_stream(&[cmd]));
    }

    #[test]
    fn test_can_stream_substitution_with_pattern_range() {
        let cmd = Command::Substitution {
            pattern: "foo".to_string(),
            replacement: "bar".to_string(),
            flags: SubstitutionFlags::default(),
            range: Some((
                Address::Pattern("start".to_string()),
                Address::Pattern("end".to_string()),
            )),
        };
        assert!(can_stream(&[cmd]));
    }

    #[test]
    fn test_can_stream_hold() {
        // Chunk 9: Hold space operations ARE streamable
        let cmd = Command::Hold { range: None };
        assert!(can_stream(&[cmd]));
    }

    #[test]
    fn test_can_stream_hold_append() {
        // Chunk 9: Hold space operations ARE streamable
        let cmd = Command::HoldAppend { range: None };
        assert!(can_stream(&[cmd]));
    }

    #[test]
    fn test_can_stream_get() {
        // Chunk 9: Hold space operations ARE streamable
        let cmd = Command::Get { range: None };
        assert!(can_stream(&[cmd]));
    }

    #[test]
    fn test_can_stream_exchange() {
        // Chunk 9: Hold space operations ARE streamable
        let cmd = Command::Exchange { range: None };
        assert!(can_stream(&[cmd]));
    }

    #[test]
    fn test_can_stream_delete() {
        let cmd = Command::Delete {
            range: (Address::LineNumber(1), Address::LineNumber(10)),
        };
        assert!(can_stream(&[cmd]));
    }

    #[test]
    fn test_can_stream_print() {
        let cmd = Command::Print {
            range: (Address::LineNumber(1), Address::LineNumber(10)),
        };
        assert!(can_stream(&[cmd]));
    }

    #[test]
    fn test_can_stream_quit() {
        let cmd = Command::Quit {
            address: Some(Address::LineNumber(10)),
        };
        assert!(can_stream(&[cmd]));
    }

    #[test]
    fn test_can_stream_insert() {
        let cmd = Command::Insert {
            text: "new line".to_string(),
            address: Address::LineNumber(5),
        };
        assert!(can_stream(&[cmd]));
    }

    #[test]
    fn test_can_stream_group_with_range() {
        // Chunk 10: Groups with streamable ranges ARE streamable
        let cmd = Command::Group {
            commands: vec![Command::Substitution {
                pattern: "foo".to_string(),
                replacement: "bar".to_string(),
                flags: SubstitutionFlags::default(),
                range: None,
            }],
            range: Some((Address::LineNumber(1), Address::LineNumber(10))),
        };
        assert!(can_stream(&[cmd]));
    }

    #[test]
    fn test_can_stream_group_without_range() {
        let cmd = Command::Group {
            commands: vec![Command::Substitution {
                pattern: "foo".to_string(),
                replacement: "bar".to_string(),
                flags: SubstitutionFlags::default(),
                range: None,
            }],
            range: None,
        };
        assert!(can_stream(&[cmd]));
    }

    #[test]
    fn test_is_range_streamable_line_to_line() {
        let range = (Address::LineNumber(1), Address::LineNumber(10));
        assert!(is_range_streamable(&range));
    }

    #[test]
    fn test_is_range_streamable_first_to_last() {
        let range = (Address::LineNumber(1), Address::LastLine);
        assert!(is_range_streamable(&range));
    }

    #[test]
    fn test_is_range_streamable_pattern_to_pattern() {
        let range = (
            Address::Pattern("start".to_string()),
            Address::Pattern("end".to_string()),
        );
        assert!(is_range_streamable(&range));
    }

    #[test]
    fn test_is_range_streamable_pattern_to_line() {
        let range = (
            Address::Pattern("start".to_string()),
            Address::LineNumber(10),
        );
        assert!(is_range_streamable(&range));
    }

    #[test]
    fn test_is_range_streamable_line_to_pattern() {
        let range = (Address::LineNumber(5), Address::Pattern("end".to_string()));
        assert!(is_range_streamable(&range));
    }

    #[test]
    fn test_is_range_streamable_pattern_to_relative() {
        let range = (
            Address::Pattern("start".to_string()),
            Address::Relative {
                base: Box::new(Address::Pattern("start".to_string())),
                offset: 5,
            },
        );
        assert!(is_range_streamable(&range));
    }

    #[test]
    fn test_is_range_streamable_stepping() {
        let range = (Address::Step { start: 1, step: 2 }, Address::LineNumber(10));
        assert!(is_range_streamable(&range));
    }

    #[test]
    fn test_is_range_not_streamable_negated() {
        let range = (
            Address::Negated(Box::new(Address::Pattern("foo".to_string()))),
            Address::LineNumber(10),
        );
        assert!(!is_range_streamable(&range));
    }

    #[test]
    fn test_is_range_not_streamable_last_line_start() {
        let range = (Address::LastLine, Address::LineNumber(10));
        assert!(!is_range_streamable(&range));
    }

    #[test]
    fn test_can_stream_multiple_commands_with_hold() {
        // Chunk 9: Hold space operations ARE streamable with other commands
        let cmds = vec![
            Command::Substitution {
                pattern: "foo".to_string(),
                replacement: "bar".to_string(),
                flags: SubstitutionFlags::default(),
                range: None,
            },
            Command::Hold { range: None },
        ];
        assert!(can_stream(&cmds));
    }

    #[test]
    fn test_can_stream_multiple_streamable_commands() {
        let cmds = vec![
            Command::Substitution {
                pattern: "foo".to_string(),
                replacement: "bar".to_string(),
                flags: SubstitutionFlags::default(),
                range: None,
            },
            Command::Delete {
                range: (Address::LineNumber(5), Address::LineNumber(10)),
            },
        ];
        assert!(can_stream(&cmds));
    }
}
