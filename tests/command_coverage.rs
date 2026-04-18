//! One end-to-end smoke test per `Command` variant. Catches parser↔executor
//! wiring bugs that unit tests cannot reach.

mod common;

use common::{read_file, sedx_isolated, write_file};
use tempfile::TempDir;

#[test]
fn pattern_address_insert_modifies_file() {
    // Regression: pattern-address i\/a\/c used to be routed to a streaming
    // path that silently dropped the command. Now routed to in-memory.
    let home = TempDir::new().unwrap();
    let input = home.path();
    let file = write_file(input, "in.txt", "alpha\nbravo\ncharlie\n");

    sedx_isolated(home.path())
        .args([r"/bravo/i\INSERTED", file.to_str().unwrap()])
        .assert()
        .success();

    assert_eq!(read_file(&file), "alpha\nINSERTED\nbravo\ncharlie\n");
}
