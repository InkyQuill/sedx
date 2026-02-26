# SedX Test Suite

## Quick Start

```bash
# Build SedX
cargo build --release

# Run quick tests (fast feedback)
./tests/run_quick_tests.sh

# Run all tests
./tests/run_all_tests.sh
```

## Test Structure

```
tests/
├── fixtures/          # Pre-generated test data (.inp = input, .good = expected)
│   ├── basic/        # Basic commands (s, d, p, q)
│   ├── addressing/   # Addressing modes and ranges
│   ├── regex/        # Regex flavor tests (PCRE, ERE, BRE)
│   ├── streaming/    # Large file tests
│   ├── pipeline/     # stdin/stdout tests
│   ├── edge/         # Edge cases
│   └── advanced/     # Advanced features
├── scripts/          # Test execution scripts
├── run_all_tests.sh  # Master test runner
└── run_quick_tests.sh # Quick test runner
```

## Documentation

- **TEST_SUITE.md** - Comprehensive test suite documentation
- **KNOWN_ISSUES.md** - Known issues and unimplemented features
- **PHASE4_TEST_RESULTS.md** - Phase 4 test results
- **CLAUDE.md** - SedX development guide

## Test Scripts

| Script | Description | Status |
|--------|-------------|--------|
| `basic_tests.sh` | Core sed commands | Partial |
| `addressing_tests.sh` | Addressing modes | Partial |
| `regex_tests.sh` | PCRE/ERE/BRE tests | PCRE only |
| `streaming_tests.sh` | Large file tests | Working |
| `pipeline_tests.sh` | stdin/stdout tests | Working |
| `edge_tests.sh` | Edge cases | Partial |
| `holdspace_tests.sh` | Hold space (h/H/g/G/x) | Untested |

## Running Tests

### All Tests

```bash
./tests/run_all_tests.sh
```

### Individual Suites

```bash
./tests/scripts/basic_tests.sh
./tests/scripts/streaming_tests.sh
./tests/scripts/pipeline_tests.sh
```

### Custom Binary

```bash
SEDX_BIN=./target/debug/sedx ./tests/run_all_tests.sh
```

## Test Fixtures

Test fixtures follow GNU sed conventions:
- `.inp` files - Input data
- `.good` files - Expected output

Fixtures are pre-generated to ensure reproducible tests.

## Current Status

✅ **Working**: Substitution, deletion, print, quit, addressing, PCRE regex, streaming, pipeline mode
⚠️ **Partial**: Negation, relative ranges, ERE/BRE modes
❌ **Not Working**: Insert/append/change commands, Unicode patterns

See `KNOWN_ISSUES.md` for details.

## Adding Tests

1. Create `.inp` file with test data
2. Run SedX to generate output
3. Save output as `.good` file
4. Add test to appropriate script in `scripts/`

See `TEST_SUITE.md` for detailed guide.

## Regression Tests

For GNU sed compatibility, see:
```bash
./tests/regression_tests.sh
```
