//! Unified Parser for Sed Commands
//!
//! Thin wrapper around `sed_parser::parse_sed_expression` that applies
//! a regex-flavor post-pass over the parsed Commands. The post-pass
//! affects only Substitution variants (pattern + replacement); other
//! variants pass through untouched. Group commands are recursed into
//! so nested substitutions inside `{ … }` are also converted.

use crate::cli::RegexFlavor;
use crate::command::Command;
use anyhow::Result;

/// Unified parser that supports sed syntax with configurable regex flavor.
pub struct Parser {
    regex_flavor: RegexFlavor,
}

impl Parser {
    pub fn new(regex_flavor: RegexFlavor) -> Self {
        Self { regex_flavor }
    }

    pub fn parse(&self, expression: &str) -> Result<Vec<Command>> {
        let mut commands = crate::sed_parser::parse_sed_expression(expression)?;
        for cmd in &mut commands {
            self.apply_flavor_to_substitutions(cmd);
        }
        Ok(commands)
    }

    /// Recursively walks a Command tree and, for every Substitution
    /// variant, rewrites its pattern and replacement into the canonical
    /// PCRE form understood by the downstream regex engine.
    fn apply_flavor_to_substitutions(&self, cmd: &mut Command) {
        match cmd {
            Command::Substitution {
                pattern,
                replacement,
                ..
            } => {
                *pattern = self.convert_pattern(pattern);
                *replacement = self.convert_replacement(replacement);
            }
            Command::Group { commands, .. } => {
                for inner in commands {
                    self.apply_flavor_to_substitutions(inner);
                }
            }
            _ => {}
        }
    }

    fn convert_pattern(&self, pattern: &str) -> String {
        match self.regex_flavor {
            RegexFlavor::BRE => crate::bre_converter::convert_bre_to_pcre(pattern),
            RegexFlavor::ERE => crate::ere_converter::convert_ere_to_pcre_pattern(pattern),
            RegexFlavor::PCRE => pattern.to_string(),
        }
    }

    fn convert_replacement(&self, replacement: &str) -> String {
        match self.regex_flavor {
            RegexFlavor::BRE => crate::bre_converter::convert_sed_backreferences(replacement),
            RegexFlavor::ERE => crate::ere_converter::convert_ere_backreferences(replacement),
            RegexFlavor::PCRE => replacement.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{Address, Command};

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
            Command::Substitution {
                pattern,
                replacement,
                flags,
                ..
            } => {
                assert_eq!(pattern, "foo");
                assert_eq!(replacement, "bar");
                assert!(!flags.global); // 'g' flag not specified
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
}
