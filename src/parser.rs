//! Unified Parser for Sed Commands
//!
//! This module provides parsing for traditional sed expressions with
//! configurable regex flavor support (PCRE, ERE, BRE).

use crate::command::{Command, Address, SubstitutionFlags};
use crate::cli::RegexFlavor;
use crate::sed_parser::{SedCommand as LegacySedCommand, Address as LegacyAddress};
use anyhow::Result;

/// Unified parser that supports sed syntax with configurable regex flavor
pub struct Parser {
    /// Regex flavor to use for parsing
    regex_flavor: RegexFlavor,
}

impl Parser {
    /// Create a new parser with the specified regex flavor
    pub fn new(regex_flavor: RegexFlavor) -> Self {
        Self { regex_flavor }
    }

    /// Parse a sed expression into unified Command list
    pub fn parse(&self, expression: &str) -> Result<Vec<Command>> {
        // Use existing sed_parser to parse the expression
        let legacy_commands = crate::sed_parser::parse_sed_expression(expression)?;

        // Convert LegacySedCommand to Command
        let commands = legacy_commands
            .into_iter()
            .map(|cmd| self.convert_legacy_command(cmd))
            .collect::<Result<Vec<_>>>()?;

        Ok(commands)
    }

    /// Convert legacy SedCommand to unified Command
    fn convert_legacy_command(&self, legacy: LegacySedCommand) -> Result<Command> {
        match legacy {
            LegacySedCommand::Substitution { pattern, replacement, flags, range } => {
                // Convert pattern based on regex flavor
                let pattern = self.convert_pattern(&pattern);
                let replacement = self.convert_replacement(&replacement);

                // Convert Vec<char> flags to SubstitutionFlags
                let substitution_flags = self.convert_flags(&flags);

                Ok(Command::Substitution {
                    pattern,
                    replacement,
                    flags: substitution_flags,
                    range: range.map(|(a, b)| {
                        (self.convert_address(a), self.convert_address(b))
                    }),
                })
            }
            LegacySedCommand::Delete { range } => {
                Ok(Command::Delete {
                    range: (self.convert_address(range.0), self.convert_address(range.1)),
                })
            }
            LegacySedCommand::Print { range } => {
                Ok(Command::Print {
                    range: (self.convert_address(range.0), self.convert_address(range.1)),
                })
            }
            LegacySedCommand::Quit { address } => {
                Ok(Command::Quit {
                    address: address.map(|a| self.convert_address(a)),
                })
            }
            LegacySedCommand::Insert { text, address } => {
                Ok(Command::Insert {
                    text,
                    address: self.convert_address(address),
                })
            }
            LegacySedCommand::Append { text, address } => {
                Ok(Command::Append {
                    text,
                    address: self.convert_address(address),
                })
            }
            LegacySedCommand::Change { text, address } => {
                Ok(Command::Change {
                    text,
                    address: self.convert_address(address),
                })
            }
            LegacySedCommand::Group { range, commands } => {
                let converted_commands = commands
                    .into_iter()
                    .map(|cmd| self.convert_legacy_command(cmd))
                    .collect::<Result<Vec<_>>>()?;

                Ok(Command::Group {
                    commands: converted_commands,
                    range: range.map(|(a, b)| {
                        (self.convert_address(a), self.convert_address(b))
                    }),
                })
            }
            LegacySedCommand::Hold { range } => {
                Ok(Command::Hold {
                    range: range.map(|(a, b)| {
                        (self.convert_address(a), self.convert_address(b))
                    }),
                })
            }
            LegacySedCommand::HoldAppend { range } => {
                Ok(Command::HoldAppend {
                    range: range.map(|(a, b)| {
                        (self.convert_address(a), self.convert_address(b))
                    }),
                })
            }
            LegacySedCommand::Get { range } => {
                Ok(Command::Get {
                    range: range.map(|(a, b)| {
                        (self.convert_address(a), self.convert_address(b))
                    }),
                })
            }
            LegacySedCommand::GetAppend { range } => {
                Ok(Command::GetAppend {
                    range: range.map(|(a, b)| {
                        (self.convert_address(a), self.convert_address(b))
                    }),
                })
            }
            LegacySedCommand::Exchange { range } => {
                Ok(Command::Exchange {
                    range: range.map(|(a, b)| {
                        (self.convert_address(a), self.convert_address(b))
                    }),
                })
            }
        }
    }

    /// Convert legacy Address to unified Address
    fn convert_address(&self, legacy: LegacyAddress) -> Address {
        match legacy {
            LegacyAddress::LineNumber(n) => Address::LineNumber(n),
            LegacyAddress::Pattern(s) => Address::Pattern(s),
            LegacyAddress::FirstLine => Address::FirstLine,
            LegacyAddress::LastLine => Address::LastLine,
            LegacyAddress::Negated(a) => Address::Negated(Box::new(self.convert_address(*a))),
            LegacyAddress::Relative { base, offset } => Address::Relative {
                base: Box::new(self.convert_address(*base)),
                offset,
            },
            LegacyAddress::Step { start, step } => Address::Step { start, step },
        }
    }

    /// Convert Vec<char> flags to SubstitutionFlags
    fn convert_flags(&self, flags: &[char]) -> SubstitutionFlags {
        let mut result = SubstitutionFlags::default();

        for flag in flags {
            match flag {
                'g' => result.global = true,
                'p' => result.print = true,
                'i' | 'I' => result.case_insensitive = true,
                '0'..='9' => {
                    // Nth occurrence flag (e.g., 2 for second occurrence)
                    let n = flag.to_digit(10).unwrap() as usize;
                    result.nth = Some(n);
                }
                _ => {} // Ignore unknown flags
            }
        }

        result
    }

    /// Convert pattern based on regex flavor to PCRE
    fn convert_pattern(&self, pattern: &str) -> String {
        match self.regex_flavor {
            RegexFlavor::BRE => {
                // BRE needs to be converted to PCRE
                crate::bre_converter::convert_bre_to_pcre(pattern)
            }
            RegexFlavor::ERE => {
                // ERE needs to be converted to PCRE (mostly pass-through)
                crate::ere_converter::convert_ere_to_pcre_pattern(pattern)
            }
            RegexFlavor::PCRE => {
                // Already PCRE, no conversion needed
                pattern.to_string()
            }
        }
    }

    /// Convert replacement based on regex flavor to PCRE format
    fn convert_replacement(&self, replacement: &str) -> String {
        match self.regex_flavor {
            RegexFlavor::BRE => {
                // BRE uses \1, \2 for backreferences → convert to $1, $2
                crate::bre_converter::convert_sed_backreferences(replacement)
            }
            RegexFlavor::ERE => {
                // ERE uses \1, \2 for backreferences → convert to $1, $2
                crate::ere_converter::convert_ere_backreferences(replacement)
            }
            RegexFlavor::PCRE => {
                // Already PCRE format with $1, $2
                replacement.to_string()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_creates_with_flavor() {
        let parser_pcre = Parser::new(RegexFlavor::PCRE);
        let parser_ere = Parser::new(RegexFlavor::ERE);
        let parser_bre = Parser::new(RegexFlavor::BRE);

        assert_eq!(parser_pcre.regex_flavor, RegexFlavor::PCRE);
        assert_eq!(parser_ere.regex_flavor, RegexFlavor::ERE);
        assert_eq!(parser_bre.regex_flavor, RegexFlavor::BRE);
    }

    #[test]
    fn test_parse_simple_substitution_pcre() {
        let parser = Parser::new(RegexFlavor::PCRE);
        let result = parser.parse("s/foo/bar/");
        assert!(result.is_ok());

        let commands = result.unwrap();
        assert_eq!(commands.len(), 1);

        match &commands[0] {
            Command::Substitution { pattern, replacement, flags, .. } => {
                assert_eq!(pattern, "foo");
                assert_eq!(replacement, "bar");
                assert!(!flags.global);  // 'g' flag not specified
            }
            _ => panic!("Expected Substitution command"),
        }
    }

    #[test]
    fn test_parse_substitution_with_flags() {
        let parser = Parser::new(RegexFlavor::PCRE);
        let result = parser.parse("s/foo/bar/gi");
        assert!(result.is_ok());

        let commands = result.unwrap();
        match &commands[0] {
            Command::Substitution { flags, .. } => {
                assert!(flags.global);
                assert!(flags.case_insensitive);
            }
            _ => panic!("Expected Substitution command"),
        }
    }

    #[test]
    fn test_parse_delete() {
        let parser = Parser::new(RegexFlavor::PCRE);
        let result = parser.parse("1,10d");
        assert!(result.is_ok());

        let commands = result.unwrap();
        match &commands[0] {
            Command::Delete { range } => {
                assert_eq!(range, &(Address::LineNumber(1), Address::LineNumber(10)));
            }
            _ => panic!("Expected Delete command"),
        }
    }

    #[test]
    fn test_parse_group() {
        let parser = Parser::new(RegexFlavor::PCRE);
        let result = parser.parse("{s/foo/bar/; s/baz/qux/}");
        assert!(result.is_ok());

        let commands = result.unwrap();
        match &commands[0] {
            Command::Group { commands, .. } => {
                assert_eq!(commands.len(), 2);
            }
            _ => panic!("Expected Group command"),
        }
    }

    #[test]
    fn test_convert_pattern_bre() {
        let parser = Parser::new(RegexFlavor::BRE);

        // BRE patterns should be converted to PCRE
        assert_eq!(parser.convert_pattern(r#"\(foo\)"#), "(foo)");
        assert_eq!(parser.convert_pattern(r#"foo\+"#), "foo+");
        assert_eq!(parser.convert_pattern(r#"foo\|bar"#), "foo|bar");
    }

    #[test]
    fn test_convert_pattern_ere() {
        let parser = Parser::new(RegexFlavor::ERE);

        // ERE patterns should pass through (already PCRE-compatible)
        assert_eq!(parser.convert_pattern(r#"(foo)"#), "(foo)");
        assert_eq!(parser.convert_pattern(r#"foo+"#), "foo+");
        assert_eq!(parser.convert_pattern(r#"foo|bar"#), "foo|bar");
    }

    #[test]
    fn test_convert_pattern_pcre() {
        let parser = Parser::new(RegexFlavor::PCRE);

        // PCRE patterns should pass through unchanged
        assert_eq!(parser.convert_pattern(r#"(foo)"#), "(foo)");
        assert_eq!(parser.convert_pattern(r#"foo+"#), "foo+");
        assert_eq!(parser.convert_pattern(r#"foo|bar"#), "foo|bar");
    }

    #[test]
    fn test_convert_replacement_bre() {
        let parser = Parser::new(RegexFlavor::BRE);

        // BRE replacements should convert backreferences to PCRE format
        assert_eq!(parser.convert_replacement(r#"\1"#), "$1");
        assert_eq!(parser.convert_replacement(r#"\2\1"#), "$2$1");
        assert_eq!(parser.convert_replacement(r#"\&"#), "$&");
    }

    #[test]
    fn test_convert_replacement_ere() {
        let parser = Parser::new(RegexFlavor::ERE);

        // ERE replacements should convert backreferences to PCRE format
        assert_eq!(parser.convert_replacement(r#"\1"#), "$1");
        assert_eq!(parser.convert_replacement(r#"\2\1"#), "$2$1");
        assert_eq!(parser.convert_replacement(r#"\&"#), "$&");
    }

    #[test]
    fn test_convert_replacement_pcre() {
        let parser = Parser::new(RegexFlavor::PCRE);

        // PCRE replacements should pass through unchanged
        assert_eq!(parser.convert_replacement(r#"$1"#), "$1");
        assert_eq!(parser.convert_replacement(r#"$2$1"#), "$2$1");
        assert_eq!(parser.convert_replacement(r#"$&"#), "$&");
    }

    #[test]
    fn test_convert_flags() {
        let parser = Parser::new(RegexFlavor::PCRE);

        let flags = parser.convert_flags(&['g', 'p', 'i']);
        assert!(flags.global);
        assert!(flags.print);
        assert!(flags.case_insensitive);

        let flags_nth = parser.convert_flags(&['g', '2']);
        assert!(flags_nth.global);
        assert_eq!(flags_nth.nth, Some(2));
    }
}
