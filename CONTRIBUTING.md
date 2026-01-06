# Contributing to SedX

Thank you for your interest in contributing to SedX! This document provides guidelines for contributing to the project.

## How to Contribute

### Reporting Bugs

Before creating bug reports, please check existing issues to avoid duplicates. When creating a bug report, include:

* A clear description of the problem
* Steps to reproduce the issue
* Expected behavior vs. actual behavior
* Your OS and Rust version (`rustc --version`)
* Example input/output if applicable

### Suggesting Enhancements

Enhancement suggestions are welcome! Please provide:

* A clear description of the proposed feature
* Use cases and benefits
* Examples of how the feature would work
* Potential implementation considerations (if known)

### Pull Requests

1. **Fork the repository** and create your branch from `main`.
2. **Install dependencies**: `cargo build`
3. **Make your changes**:
   - Write code following Rust best practices
   - Add tests for new functionality
   - Ensure all tests pass: `cargo test`
   - Format your code: `cargo fmt`
   - Run linter: `cargo clippy -- -D warnings`
4. **Commit your changes** with clear, descriptive messages
5. **Push to your fork** and submit a pull request

### Development Setup

```bash
# Clone the repository
git clone https://github.com/InkyQuill/sedx.git
cd sedx

# Build in debug mode
cargo build

# Run tests
cargo test

# Run with optimizations
cargo build --release
./target/release/sedx --help
```

### Code Style

* Follow Rust naming conventions and idioms
* Use meaningful variable and function names
* Add comments for complex logic
* Keep functions focused and concise
* Write documentation for public APIs

### Testing

We value comprehensive testing:

* Write unit tests for new functionality
* Add integration tests for complex scenarios
* Ensure backward compatibility with existing sed features
* Test edge cases (empty files, single lines, invalid patterns)

Run tests with:
```bash
# All tests
cargo test

# Run specific test
cargo test test_name

# Run with output
cargo test -- --nocapture

# Run integration tests
./tests/regression_tests.sh
```

### Project Structure

```
sedx/
├── src/
│   ├── main.rs              # CLI entry point
│   ├── cli.rs               # Argument parsing
│   ├── sed_parser.rs        # Sed expression parser
│   ├── file_processor.rs    # File processing logic
│   ├── diff_formatter.rs    # Output formatting
│   └── backup_manager.rs    # Backup/rollback system
├── tests/
│   └── regression_tests.sh  # Integration tests
└── Cargo.toml
```

### Adding New Sed Features

When adding sed features:

1. Research GNU sed behavior thoroughly
2. Update `sed_parser.rs` to parse the new command
3. Implement logic in `file_processor.rs`
4. Add comprehensive tests comparing with GNU sed
5. Update README.md with examples
6. Ensure existing tests still pass

## Style Guidelines

* Use `cargo fmt` before committing
* Address clippy warnings: `cargo clippy`
* Keep commits atomic and focused
* Write descriptive commit messages

## Questions?

Feel free to open an issue for discussion before making large changes. We're happy to guide you!

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
