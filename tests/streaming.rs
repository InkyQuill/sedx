//! Tests that the streaming processor produces byte-exact output on both
//! small files (forced via --streaming) and large files (#[ignore]'d).

mod common;

use common::{read_file, sedx_isolated, write_file};
use sedx::cli::RegexFlavor;
use sedx::file_processor::StreamProcessor;
use sedx::parser::Parser;
use std::io::Cursor;
use tempfile::TempDir;

#[test]
fn forced_streaming_small_file_matches_in_memory() {
    let home = TempDir::new().unwrap();
    let dir = home.path();
    let input: String = (0..1000).map(|i| format!("line {} foo\n", i)).collect();

    let in_mem = write_file(dir, "mem.txt", &input);
    sedx_isolated(dir)
        .args(["--no-streaming", "s/foo/bar/", in_mem.to_str().unwrap()])
        .assert()
        .success();

    let streamed = write_file(dir, "stream.txt", &input);
    sedx_isolated(dir)
        .args(["--streaming", "s/foo/bar/", streamed.to_str().unwrap()])
        .assert()
        .success();

    let mem_out = read_file(&in_mem);
    let stream_out = read_file(&streamed);

    // Paths must agree on the output bytes.
    assert_eq!(mem_out, stream_out);

    // Positive check: the substitution actually ran (both paths broken the
    // same way would satisfy only the equality above).
    assert!(!mem_out.contains("foo"));
    assert_eq!(mem_out.matches("bar").count(), 1000);
}

#[test]
fn no_streaming_handles_small_non_streamable_commands() {
    let home = TempDir::new().unwrap();
    let dir = home.path();
    let file = write_file(dir, "change.txt", "alpha\nbravo\ncharlie\n");

    sedx_isolated(dir)
        .args(["--no-streaming", r"/bravo/c\BRAVO", file.to_str().unwrap()])
        .assert()
        .success();

    assert_eq!(read_file(&file), "alpha\nBRAVO\ncharlie\n");
}

#[test]
fn forced_streaming_last_line_delete_removes_only_final_line() {
    let home = TempDir::new().unwrap();
    let dir = home.path();
    let file = write_file(dir, "last-delete.txt", "alpha\nbravo\ncharlie\n");

    sedx_isolated(dir)
        .args(["--streaming", "$d", file.to_str().unwrap()])
        .assert()
        .success();

    assert_eq!(read_file(&file), "alpha\nbravo\n");
}

#[test]
fn streaming_last_line_quit_outputs_all_lines() {
    let parser = Parser::new(RegexFlavor::PCRE);
    let commands = parser.parse("$q").unwrap();
    let mut processor = StreamProcessor::new(commands);
    let mut output = Vec::new();

    processor
        .process_reader_to_writer(Cursor::new("alpha\nbravo\ncharlie\n"), &mut output, "stdin")
        .unwrap();

    assert_eq!(
        String::from_utf8(output).unwrap(),
        "alpha\nbravo\ncharlie\n"
    );
}

#[test]
fn streaming_clear_pattern_space_emits_empty_pattern_spaces() {
    let parser = Parser::new(RegexFlavor::PCRE);
    let commands = parser.parse("z").unwrap();
    let mut processor = StreamProcessor::new(commands);
    let mut output = Vec::new();

    processor
        .process_reader_to_writer(Cursor::new("alpha\nbravo\ncharlie\n"), &mut output, "stdin")
        .unwrap();

    assert_eq!(String::from_utf8(output).unwrap(), "\n\n\n");
}

#[test]
fn streaming_pattern_address_insert_append_and_change() {
    let parser = Parser::new(RegexFlavor::PCRE);
    let input = "alpha\nbravo\ncharlie\n";

    let commands = parser.parse(r"/bravo/i\BEFORE").unwrap();
    let mut processor = StreamProcessor::new(commands);
    let mut output = Vec::new();
    processor
        .process_reader_to_writer(Cursor::new(input), &mut output, "stdin")
        .unwrap();
    assert_eq!(
        String::from_utf8(output).unwrap(),
        "alpha\nBEFORE\nbravo\ncharlie\n"
    );

    let commands = parser.parse(r"/bravo/a\AFTER").unwrap();
    let mut processor = StreamProcessor::new(commands);
    let mut output = Vec::new();
    processor
        .process_reader_to_writer(Cursor::new(input), &mut output, "stdin")
        .unwrap();
    assert_eq!(
        String::from_utf8(output).unwrap(),
        "alpha\nbravo\nAFTER\ncharlie\n"
    );

    let commands = parser.parse(r"/bravo/c\REPLACED").unwrap();
    let mut processor = StreamProcessor::new(commands);
    let mut output = Vec::new();
    processor
        .process_reader_to_writer(Cursor::new(input), &mut output, "stdin")
        .unwrap();
    assert_eq!(
        String::from_utf8(output).unwrap(),
        "alpha\nREPLACED\ncharlie\n"
    );
}

#[test]
fn streaming_last_line_insert_append_and_change() {
    let parser = Parser::new(RegexFlavor::PCRE);
    let input = "alpha\nbravo\ncharlie\n";

    let commands = parser.parse(r"$i\BEFORE LAST").unwrap();
    let mut processor = StreamProcessor::new(commands);
    let mut output = Vec::new();
    processor
        .process_reader_to_writer(Cursor::new(input), &mut output, "stdin")
        .unwrap();
    assert_eq!(
        String::from_utf8(output).unwrap(),
        "alpha\nbravo\nBEFORE LAST\ncharlie\n"
    );

    let commands = parser.parse(r"$a\AFTER LAST").unwrap();
    let mut processor = StreamProcessor::new(commands);
    let mut output = Vec::new();
    processor
        .process_reader_to_writer(Cursor::new(input), &mut output, "stdin")
        .unwrap();
    assert_eq!(
        String::from_utf8(output).unwrap(),
        "alpha\nbravo\ncharlie\nAFTER LAST\n"
    );

    let commands = parser.parse(r"$c\LAST").unwrap();
    let mut processor = StreamProcessor::new(commands);
    let mut output = Vec::new();
    processor
        .process_reader_to_writer(Cursor::new(input), &mut output, "stdin")
        .unwrap();
    assert_eq!(String::from_utf8(output).unwrap(), "alpha\nbravo\nLAST\n");
}

#[test]
fn forced_streaming_accepts_pattern_address_insert_append_and_change() {
    let home = TempDir::new().unwrap();
    let dir = home.path();

    let insert = write_file(dir, "insert.txt", "alpha\nbravo\ncharlie\n");
    sedx_isolated(dir)
        .args(["--streaming", r"/bravo/i\BEFORE", insert.to_str().unwrap()])
        .assert()
        .success();
    assert_eq!(read_file(&insert), "alpha\nBEFORE\nbravo\ncharlie\n");

    let append = write_file(dir, "append.txt", "alpha\nbravo\ncharlie\n");
    sedx_isolated(dir)
        .args(["--streaming", r"/bravo/a\AFTER", append.to_str().unwrap()])
        .assert()
        .success();
    assert_eq!(read_file(&append), "alpha\nbravo\nAFTER\ncharlie\n");

    let change = write_file(dir, "change.txt", "alpha\nbravo\ncharlie\n");
    sedx_isolated(dir)
        .args([
            "--streaming",
            r"/bravo/c\REPLACED",
            change.to_str().unwrap(),
        ])
        .assert()
        .success();
    assert_eq!(read_file(&change), "alpha\nREPLACED\ncharlie\n");
}

#[test]
fn streaming_next_append_substitution_processes_pairs() {
    let parser = Parser::new(RegexFlavor::PCRE);
    let commands = parser.parse(r"N; s/\n/ /").unwrap();
    let mut processor = StreamProcessor::new(commands);
    let mut output = Vec::new();

    processor
        .process_reader_to_writer(Cursor::new("a\nb\nc\nd\n"), &mut output, "stdin")
        .unwrap();

    assert_eq!(String::from_utf8(output).unwrap(), "a b\nc d\n");
}

#[test]
fn streaming_next_append_at_eof_preserves_odd_final_line() {
    let parser = Parser::new(RegexFlavor::PCRE);
    let commands = parser.parse(r"N; s/\n/ /").unwrap();
    let mut processor = StreamProcessor::new(commands);
    let mut output = Vec::new();

    processor
        .process_reader_to_writer(Cursor::new("a\nb\nc\n"), &mut output, "stdin")
        .unwrap();

    assert_eq!(String::from_utf8(output).unwrap(), "a b\nc\n");
}

#[test]
fn streaming_print_first_line_records_side_effect_output() {
    let parser = Parser::new(RegexFlavor::PCRE);
    let commands = parser.parse("N; P").unwrap();
    let mut processor = StreamProcessor::new(commands);
    let mut output = Vec::new();

    let diff = processor
        .process_reader_to_writer(Cursor::new("a\nb\nc\nd\n"), &mut output, "stdin")
        .unwrap();

    assert_eq!(String::from_utf8(output).unwrap(), "a\nb\nc\nd\n");
    assert_eq!(diff.printed_lines, ["a", "c"]);
}

#[test]
fn streaming_print_line_number_and_filename_record_side_effect_output() {
    let parser = Parser::new(RegexFlavor::PCRE);

    let commands = parser.parse("=").unwrap();
    let mut processor = StreamProcessor::new(commands);
    let mut output = Vec::new();
    let diff = processor
        .process_reader_to_writer(Cursor::new("a\nb\n"), &mut output, "input.txt")
        .unwrap();
    assert_eq!(String::from_utf8(output).unwrap(), "a\nb\n");
    assert_eq!(diff.printed_lines, ["1", "2"]);

    let commands = parser.parse("/b/F").unwrap();
    let mut processor = StreamProcessor::new(commands);
    let mut output = Vec::new();
    let diff = processor
        .process_reader_to_writer(Cursor::new("a\nb\n"), &mut output, "input.txt")
        .unwrap();
    assert_eq!(String::from_utf8(output).unwrap(), "a\nb\n");
    assert_eq!(diff.printed_lines, ["input.txt"]);
}

#[test]
fn streaming_next_replaces_pattern_space_and_continues_script() {
    let parser = Parser::new(RegexFlavor::PCRE);
    let commands = parser.parse("n; s/.*/X/").unwrap();
    let mut processor = StreamProcessor::new(commands);
    let mut output = Vec::new();

    processor
        .process_reader_to_writer(Cursor::new("a\nb\nc\nd\n"), &mut output, "stdin")
        .unwrap();

    assert_eq!(String::from_utf8(output).unwrap(), "a\nX\nc\nX\n");
}

#[test]
fn streaming_next_at_eof_stops_before_later_commands() {
    let parser = Parser::new(RegexFlavor::PCRE);
    let commands = parser.parse("n; s/.*/X/").unwrap();
    let mut processor = StreamProcessor::new(commands);
    let mut output = Vec::new();

    processor
        .process_reader_to_writer(Cursor::new("a\nb\nc\n"), &mut output, "stdin")
        .unwrap();

    assert_eq!(String::from_utf8(output).unwrap(), "a\nX\nc\n");
}

#[test]
fn streaming_change_line_range_collapses_to_single_replacement() {
    let parser = Parser::new(RegexFlavor::PCRE);
    let commands = parser.parse(r"2,4c\REPLACED").unwrap();
    let mut processor = StreamProcessor::new(commands);
    let mut output = Vec::new();

    processor
        .process_reader_to_writer(
            Cursor::new("one\ntwo\nthree\nfour\nfive\n"),
            &mut output,
            "stdin",
        )
        .unwrap();

    assert_eq!(String::from_utf8(output).unwrap(), "one\nREPLACED\nfive\n");
}

#[test]
fn streaming_change_pattern_range_collapses_to_single_replacement() {
    let parser = Parser::new(RegexFlavor::PCRE);
    let commands = parser.parse(r"/start/,/end/c\REPLACED").unwrap();
    let mut processor = StreamProcessor::new(commands);
    let mut output = Vec::new();

    processor
        .process_reader_to_writer(
            Cursor::new("before\nstart\nmiddle\nend\nafter\n"),
            &mut output,
            "stdin",
        )
        .unwrap();

    assert_eq!(
        String::from_utf8(output).unwrap(),
        "before\nREPLACED\nafter\n"
    );
}

#[cfg(unix)]
#[test]
fn streaming_via_symlink_writes_to_target() {
    use std::os::unix::fs::symlink;

    let home = TempDir::new().unwrap();
    let dir = home.path();
    let target = write_file(dir, "target.txt", "foo\nbar\nbaz\n");
    let link = dir.join("link.txt");
    symlink(&target, &link).unwrap();

    sedx_isolated(dir)
        .args(["--streaming", "s/bar/BAR/", link.to_str().unwrap()])
        .assert()
        .success();

    assert!(link.symlink_metadata().unwrap().file_type().is_symlink());
    assert_eq!(read_file(&target), "foo\nBAR\nbaz\n");
}

/// Opt-in via `cargo test -- --ignored`. Generates a ~100 MB input file and
/// verifies that every line was transformed correctly. Takes ~5 s; the
/// #[ignore] keeps routine CI fast.
#[test]
#[ignore]
fn streaming_100mb_file_correctness() {
    let home = TempDir::new().unwrap();
    let dir = home.path();
    let path = dir.join("big.txt");

    // 1 million lines × ~100 bytes each ≈ 100 MB.
    let mut contents = String::with_capacity(100 * 1024 * 1024);
    for i in 0..1_000_000 {
        contents.push_str(&format!("line {:07} foo {:0>50}\n", i, "x"));
    }
    std::fs::write(&path, &contents).unwrap();

    sedx_isolated(dir)
        .args(["s/foo/bar/", path.to_str().unwrap()])
        .assert()
        .success();

    let result = std::fs::read_to_string(&path).unwrap();
    assert_eq!(result.matches(" foo ").count(), 0);
    assert_eq!(result.matches(" bar ").count(), 1_000_000);
}
