//! Unified Command System (UCS)
//!
//! This module defines the unified command representation that supports
//! both traditional sed syntax and sd-like simple find/replace syntax.

use serde::{Deserialize, Serialize};

/// Unified command representation that supports both sed and sd syntaxes
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Command {
    /// Text substitution (sed: s/pattern/replacement/flags, sd: pattern replacement)
    Substitution {
        pattern: String,
        replacement: String,
        flags: SubstitutionFlags,
        range: Option<(Address, Address)>,
    },

    /// Delete lines (sed: 1,10d)
    Delete {
        range: (Address, Address),
    },

    /// Print lines (sed: 1,10p)
    Print {
        range: (Address, Address),
    },

    /// Quit processing (sed: 10q)
    Quit {
        address: Option<Address>,
    },

    /// Quit without printing (sed: 10Q) - Phase 4
    QuitWithoutPrint {
        address: Option<Address>,
    },

    /// Insert text before line (sed: 5i\text)
    Insert {
        text: String,
        address: Address,
    },

    /// Append text after line (sed: 5a\text)
    Append {
        text: String,
        address: Address,
    },

    /// Change line (sed: 5c\text)
    Change {
        text: String,
        address: Address,
    },

    /// Command group (sed: {s/foo/bar/; p})
    Group {
        commands: Vec<Command>,
        range: Option<(Address, Address)>,
    },

    /// Hold space operation: copy pattern space to hold space
    Hold {
        range: Option<(Address, Address)>,
    },

    /// Hold append: append pattern space to hold space
    HoldAppend {
        range: Option<(Address, Address)>,
    },

    /// Get: copy hold space to pattern space
    Get {
        range: Option<(Address, Address)>,
    },

    /// Get append: append hold space to pattern space
    GetAppend {
        range: Option<(Address, Address)>,
    },

    /// Exchange: swap pattern space and hold space
    Exchange {
        range: Option<(Address, Address)>,
    },

    /// Next: print current pattern space, read next line, start new cycle (Phase 4)
    Next {
        range: Option<(Address, Address)>,
    },

    /// Next with append: read next line and append to pattern space (Phase 4)
    NextAppend {
        range: Option<(Address, Address)>,
    },

    /// Print first line: print up to first newline in pattern space (Phase 4)
    PrintFirstLine {
        range: Option<(Address, Address)>,
    },

    /// Delete first line: delete up to first newline, restart cycle (Phase 4)
    DeleteFirstLine {
        range: Option<(Address, Address)>,
    },

    /// Label definition (Phase 5): :label - defines a branch target
    Label {
        name: String,
    },

    /// Branch (Phase 5): b [label] - unconditional branch to label
    /// If no label specified, branches to end of script
    /// Can have optional address/range: addr b or addr1,addr2 b label
    Branch {
        label: Option<String>,
        range: Option<(Address, Address)>,
    },

    /// Test branch (Phase 5): t [label] - branch if substitution made
    /// Branches to label if a substitution was made since last input
    /// Can have optional address/range: addr t or addr1,addr2 t label
    Test {
        label: Option<String>,
        range: Option<(Address, Address)>,
    },

    /// Test false branch (Phase 5): T [label] - branch if NO substitution
    /// Branches to label if NO substitution was made since last input
    /// Can have optional address/range: addr T or addr1,addr2 T label
    TestFalse {
        label: Option<String>,
        range: Option<(Address, Address)>,
    },
}

/// Substitution flags (unified across sed and sd)
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SubstitutionFlags {
    /// g - all occurrences (sed: off by default, sd: on by default)
    pub global: bool,

    /// p - print substituted lines
    pub print: bool,

    /// i - case-insensitive matching
    pub case_insensitive: bool,

    /// N - substitute Nth occurrence only
    pub nth: Option<usize>,
}

/// Unified address representation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Address {
    /// Specific line number (e.g., 10)
    LineNumber(usize),

    /// Regex pattern match (e.g., /foo/)
    Pattern(String),

    /// First line (special address "0")
    FirstLine,

    /// Last line (special address "$")
    LastLine,

    /// Negated address (e.g., !10, !/pattern/)
    Negated(Box<Address>),

    /// Relative offset (e.g., /pattern/,+5)
    Relative {
        base: Box<Address>,
        offset: isize,
    },

    /// Step addressing (e.g., 1~2 for every 2nd line from line 1)
    Step {
        start: usize,
        step: usize,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_equality() {
        let cmd1 = Command::Delete {
            range: (Address::LineNumber(1), Address::LineNumber(10)),
        };
        let cmd2 = Command::Delete {
            range: (Address::LineNumber(1), Address::LineNumber(10)),
        };
        assert_eq!(cmd1, cmd2);
    }

    #[test]
    fn test_address_types() {
        let line_addr = Address::LineNumber(42);
        let pattern_addr = Address::Pattern("foo".to_string());
        let first_addr = Address::FirstLine;
        let last_addr = Address::LastLine;
        let negated_addr = Address::Negated(Box::new(Address::LineNumber(5)));
        let relative_addr = Address::Relative {
            base: Box::new(Address::Pattern("start".to_string())),
            offset: 5,
        };
        let step_addr = Address::Step { start: 1, step: 2 };

        // All should compile and be valid
        assert!(matches!(line_addr, Address::LineNumber(42)));
        assert!(matches!(pattern_addr, Address::Pattern(_)));
        assert!(matches!(first_addr, Address::FirstLine));
        assert!(matches!(last_addr, Address::LastLine));
        assert!(matches!(negated_addr, Address::Negated(_)));
        assert!(matches!(relative_addr, Address::Relative { .. }));
        assert!(matches!(step_addr, Address::Step { .. }));
    }

    #[test]
    fn test_substitution_flags_default() {
        let flags = SubstitutionFlags::default();
        assert!(!flags.global);
        assert!(!flags.print);
        assert!(!flags.case_insensitive);
        assert!(flags.nth.is_none());
    }

    #[test]
    fn test_substitution_flags_custom() {
        let flags = SubstitutionFlags {
            global: true,
            print: false,
            case_insensitive: true,
            nth: Some(3),
        };
        assert!(flags.global);
        assert!(!flags.print);
        assert!(flags.case_insensitive);
        assert_eq!(flags.nth, Some(3));
    }
}
