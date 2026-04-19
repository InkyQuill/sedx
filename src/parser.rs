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
    /// Create a parser that interprets substitution patterns and
    /// replacements in the given regex flavor.
    pub fn new(regex_flavor: RegexFlavor) -> Self {
        Self { regex_flavor }
    }

    /// Parse a sed-style expression into a flat list of commands.
    ///
    /// `sed_parser::parse_sed_expression` does the structural parse and
    /// emits `Command` values directly; this wrapper then walks each
    /// command and, for Substitution variants (including those nested
    /// inside `Group` commands), rewrites pattern and replacement into
    /// the canonical PCRE form the runtime regex engine expects. Errors
    /// come from the underlying parser (malformed expressions); the
    /// flavor-conversion pass is infallible.
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
            RegexFlavor::ERE => crate::ere_converter::convert_ere_backreferences(replacement),
            // BRE and PCRE both accept GNU-sed-style replacements (\1, \&,
            // \\) and convert them to the regex crate's form ($1, $&, \).
            // PCRE keeps this convenience so existing sed scripts work
            // unchanged when upgrading from `sed` to `sedx`.
            RegexFlavor::BRE | RegexFlavor::PCRE => {
                crate::bre_converter::convert_sed_backreferences(replacement)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{Address, Command, SubstitutionFlags};

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

        // Canonical PCRE replacements (already $N form) pass through.
        assert_eq!(parser.convert_replacement(r#"$1"#), "$1");
        assert_eq!(parser.convert_replacement(r#"$2$1"#), "$2$1");
        assert_eq!(parser.convert_replacement(r#"$&"#), "$&");

        // GNU-sed-style \N backreferences also get converted under PCRE
        // — a convenience feature so existing sed scripts work unchanged
        // in the default flavor. See convert_replacement's doc-comment.
        assert_eq!(parser.convert_replacement(r#"\1"#), "$1");
        assert_eq!(parser.convert_replacement(r#"\2\1"#), "$2$1");
        assert_eq!(parser.convert_replacement(r#"\&"#), "$&");
    }

    fn substitution(pattern: &str, replacement: &str) -> Command {
        Command::Substitution {
            pattern: pattern.to_string(),
            replacement: replacement.to_string(),
            flags: SubstitutionFlags::default(),
            range: None,
        }
    }

    #[test]
    fn pcre_flavor_is_pass_through() {
        let parser = Parser::new(RegexFlavor::PCRE);
        let mut cmd = substitution(r"(foo)(bar)", "$2$1");
        parser.apply_flavor_to_substitutions(&mut cmd);
        match cmd {
            Command::Substitution {
                pattern,
                replacement,
                ..
            } => {
                assert_eq!(pattern, "(foo)(bar)");
                assert_eq!(replacement, "$2$1");
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn ere_flavor_converts_backslash_backrefs() {
        let parser = Parser::new(RegexFlavor::ERE);
        // ERE pattern passes through (it's already PCRE-compatible);
        // only the replacement's \1/\2 gets rewritten to $1/$2.
        let mut cmd = substitution("(foo)(bar)", r"\2\1");
        parser.apply_flavor_to_substitutions(&mut cmd);
        match cmd {
            Command::Substitution {
                pattern,
                replacement,
                ..
            } => {
                assert_eq!(pattern, "(foo)(bar)");
                assert_eq!(replacement, "$2$1");
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn bre_flavor_converts_pattern_and_backrefs() {
        let parser = Parser::new(RegexFlavor::BRE);
        // BRE pattern `\(foo\)` becomes `(foo)`; `\1` becomes `$1`.
        let mut cmd = substitution(r"\(foo\)", r"\1");
        parser.apply_flavor_to_substitutions(&mut cmd);
        match cmd {
            Command::Substitution {
                pattern,
                replacement,
                ..
            } => {
                assert_eq!(pattern, "(foo)");
                assert_eq!(replacement, "$1");
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn group_commands_recurse_into_nested_substitutions() {
        let parser = Parser::new(RegexFlavor::BRE);
        let mut group = Command::Group {
            commands: vec![substitution(r"\(a\)", r"\1"), substitution(r"\(b\)", r"\1")],
            range: None,
        };
        parser.apply_flavor_to_substitutions(&mut group);
        match group {
            Command::Group { commands, .. } => {
                match &commands[0] {
                    Command::Substitution {
                        pattern,
                        replacement,
                        ..
                    } => {
                        assert_eq!(pattern, "(a)");
                        assert_eq!(replacement, "$1");
                    }
                    _ => unreachable!(),
                }
                match &commands[1] {
                    Command::Substitution {
                        pattern,
                        replacement,
                        ..
                    } => {
                        assert_eq!(pattern, "(b)");
                        assert_eq!(replacement, "$1");
                    }
                    _ => unreachable!(),
                }
            }
            _ => unreachable!(),
        }
    }
}
