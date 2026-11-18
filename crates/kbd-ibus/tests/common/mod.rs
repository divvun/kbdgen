//! What the behaviour tests share: kbdgen's fixture layouts compiled for
//! Linux (`fixture`), a fake client that applies the engine's signals
//! (`field`), the engine run natively (`native`), and the typing cases of
//! `linux.test.planner` (`cases`).

// Each test binary uses part of this module.
#![allow(dead_code)]
// Clippy counts only `#[test]` functions of a test binary as tests, not
// the helpers beside them.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

// [spec:kbdgen:req:linux.test.planner]
pub mod cases;
// [spec:kbdgen:req:linux.test.planner]
pub mod field;
pub mod fixture;
// [spec:kbdgen:req:linux.test.planner]
pub mod native;
