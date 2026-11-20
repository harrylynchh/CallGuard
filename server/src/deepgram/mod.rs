//! Deepgram streaming speech-to-text.

mod client;
pub mod messages;

pub use client::{DeepgramStream, ResultSender};
