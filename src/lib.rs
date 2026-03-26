#![deny(clippy::all, clippy::pedantic)]
#![deny(warnings)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::missing_errors_doc)]

pub mod app;
pub mod model;
mod pipeline_editor;
mod templates;

pub use app::TasksApp;
