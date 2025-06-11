pub mod handler;
pub mod key;

pub use handler::{AppEvent, EventConfig, EventHandler};
pub use key::Key;

/*
EXPLANATION:
- This file declares the submodules within the events module
- It re-exports the important types so other parts of our code can easily access them
- This pattern is common in Rust - create a module directory with mod.rs and submodules
*/
