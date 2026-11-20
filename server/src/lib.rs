//! CallGuard server: answers Twilio calls, streams audio to speech-to-text and
//! flags scam language live.

pub mod config;
pub mod routes;
pub mod state;
pub mod twilio;
