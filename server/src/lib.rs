//! CallGuard server: answers Twilio calls, streams audio to speech-to-text and
//! flags scam language live.

pub mod audio;
pub mod calls;
pub mod config;
pub mod deepgram;
pub mod media;
pub mod replay;
pub mod routes;
pub mod state;
pub mod twilio;
