pub mod common;
pub mod in_memory;
pub mod streaming;

pub use common::{ChangeType, FileDiff, LineChange};
pub use in_memory::FileProcessor;
pub use streaming::StreamProcessor;
