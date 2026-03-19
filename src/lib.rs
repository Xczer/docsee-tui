#![allow(clippy::uninlined_format_args)]

pub mod app;
pub mod config;
pub mod docker;
pub mod events;
pub mod theme;
pub mod ui;
pub mod widgets;

/*
EXPLANATION:
- This file declares all the modules in our project
- Each `pub mod` statement tells Rust to include that module and make it public
- This allows other parts of our code to use these modules
- Rust will look for these modules in files like app.rs or directories like events/
- Tests are only included when building for tests
*/
