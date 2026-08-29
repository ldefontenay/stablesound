//! WASAPI access: finding devices, holding the keep-alive stream open, and
//! measuring whether anything else is playing.
//!
//! COM objects here are not `Send`. Everything in this module must run on the
//! single thread that owns the audio work - see `engine`.

pub mod device;
pub mod keepalive;
pub mod meter;
