//! Short tones that say what just happened.
//!
//! CLAUDE.md: state changes are audible, never visual-only. These are the whole
//! feedback channel for the hotkey - press it and the only way to know what it
//! did is to hear it.
//!
//! # Why they play through the keep-alive stream
//!
//! The obvious design - "release the device, then play a tone" - is wrong, and
//! wrong in a way that undoes the point of the app. Opening the device again to
//! play the off-tone would take the headset straight back off the phone, which
//! is exactly what releasing it was for. So the tone is queued into the render
//! buffer of the stream that is already open, and the stream closes once it has
//! drained. See `KeepAlive::play` and `KeepAlive::drain`.
//!
//! Consequence, and a real limitation: **an earcon can only play while a stream
//! is open.** Switching off when nothing is open is silent. That case is rare -
//! if no stream is open the headset is not being held, so switching off changes
//! nothing anyway - but it is worth knowing about rather than discovering.
//!
//! # Why these shapes
//!
//! Rising for on, falling for off. That mapping is close to universal in
//! interface sound, so it needs no learning, and the direction is audible even
//! when the tone is short or the headset is still waking up. Two notes rather
//! than one, because a single beep is easy to mistake for a system sound.
//!
//! Lengths and volume are the tester's, from the Milestone 3 hardware round:
//! the first guess was a fifth too long and twice as loud as wanted.

/// One note: a frequency in Hz and a length in milliseconds.
///
/// A frequency of zero is silence - `sin(0)` is zero for every frame - which is
/// how `ON` gets its lead-in below.
#[derive(Clone, Copy, Debug)]
pub struct Note {
    pub freq: f32,
    pub ms: u32,
}

/// Silence in front of the on-tone, so the tone is not clipped.
///
/// Milestone 3 testing: "when the keep-alive stream is being switched on, the
/// complete tone is not heard". Writing samples the instant after
/// `IAudioClient::Start` is not the same as those samples reaching the ears - a
/// Bluetooth headset needs a moment to bring its link up, and whatever is
/// written during it is lost. There is no event that says "now you are actually
/// audible", so this is simply a wait long enough to cover it.
///
/// Only the on-tone needs it. The off-tone plays through a stream that has been
/// running for a while, and delaying a release is the one thing the whole
/// design refuses to do.
///
/// Halved from 250 ms after the second round, at the tester's suggestion: the
/// whole tone was audible but felt slow, and "I can hear JAWS starting to
/// announce the keystroke prior to the tone playing, so it should be safe to
/// halve the delay". That is a useful measurement as well as a preference -
/// JAWS beginning to speak means the link was already up.
const LEAD_IN_MS: u32 = 125;

/// Rising - keep-alive is on and the headset is being held awake.
pub const ON: &[Note] = &[
    Note {
        freq: 0.0,
        ms: LEAD_IN_MS,
    },
    Note {
        freq: 660.0,
        ms: 56,
    },
    Note {
        freq: 990.0,
        ms: 88,
    },
];

/// Falling - the device has been let go and another one can take it.
pub const OFF: &[Note] = &[
    Note {
        freq: 990.0,
        ms: 56,
    },
    Note {
        freq: 660.0,
        ms: 88,
    },
];

/// Renders a sequence of notes into samples, one frame at a time.
pub struct Player {
    notes: &'static [Note],
    /// Index of the note being played.
    note: usize,
    /// Frames emitted within the current note.
    frame: u64,
    sample_rate: f64,
    amplitude: f32,
}

impl Player {
    pub fn new(notes: &'static [Note], sample_rate: f64, amplitude: f32) -> Self {
        Player {
            notes,
            note: 0,
            frame: 0,
            sample_rate,
            amplitude,
        }
    }

    pub fn finished(&self) -> bool {
        self.note >= self.notes.len()
    }

    /// The next sample, or `None` once the sequence is done.
    pub fn next_sample(&mut self) -> Option<f32> {
        let note = *self.notes.get(self.note)?;
        let length = (self.sample_rate * f64::from(note.ms) / 1000.0) as u64;

        if self.frame >= length {
            self.note += 1;
            self.frame = 0;
            return self.next_sample();
        }

        let phase =
            (self.frame as f64) * f64::from(note.freq) * std::f64::consts::TAU / self.sample_rate;
        // Fade the first and last few milliseconds of each note. Starting or
        // stopping a sine mid-cycle produces a click, and on headphones a click
        // is more noticeable than the tone itself.
        let value = (phase.sin() as f32) * self.amplitude * envelope(self.frame, length);
        self.frame += 1;
        Some(value)
    }
}

/// Linear fade over roughly 5 ms at each end, shortened for very short notes.
fn envelope(frame: u64, length: u64) -> f32 {
    let fade = (length / 8).clamp(1, 240);
    let rising = (frame + 1).min(fade) as f32 / fade as f32;
    let falling = (length.saturating_sub(frame)).min(fade) as f32 / fade as f32;
    rising.min(falling)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f64 = 48_000.0;

    #[test]
    fn plays_every_note_then_stops() {
        let mut player = Player::new(ON, RATE, 0.2);
        let mut count = 0u64;
        while player.next_sample().is_some() {
            count += 1;
            assert!(count < 1_000_000, "never finished");
        }
        let expected: u64 = ON
            .iter()
            .map(|n| (RATE * f64::from(n.ms) / 1000.0) as u64)
            .sum();
        assert_eq!(count, expected);
        assert!(player.finished());
    }

    #[test]
    fn stays_within_the_requested_amplitude() {
        let mut player = Player::new(OFF, RATE, 0.25);
        while let Some(s) = player.next_sample() {
            assert!(s.abs() <= 0.25 + 1e-6, "{s} exceeded amplitude");
        }
    }

    #[test]
    fn starts_and_ends_near_silence() {
        // The anti-click envelope: a tone that begins at full amplitude
        // produces a pop, which on headphones is worse than the tone.
        let mut player = Player::new(OFF, RATE, 1.0);
        let first = player.next_sample().unwrap();
        assert!(first.abs() < 0.05, "first sample {first} was not faded in");

        let mut samples = vec![first];
        while let Some(s) = player.next_sample() {
            samples.push(s);
        }
        let last = *samples.last().unwrap();
        assert!(last.abs() < 0.05, "last sample {last} was not faded out");
    }

    #[test]
    fn on_rises_and_off_falls() {
        let audible: Vec<&Note> = ON.iter().filter(|n| n.freq > 0.0).collect();
        assert_eq!(audible.len(), 2);
        assert!(audible[1].freq > audible[0].freq);
        assert!(OFF[1].freq < OFF[0].freq);
    }

    #[test]
    fn the_on_tone_leads_with_silence_but_the_off_tone_does_not() {
        // A release that waited a quarter of a second before even starting the
        // tone would add that to every release, which is the opposite of what
        // the design is for.
        assert_eq!(ON[0].freq, 0.0);
        assert!(ON[0].ms > 0);
        assert!(OFF.iter().all(|n| n.freq > 0.0));

        let mut player = Player::new(ON, RATE, 1.0);
        let lead = (RATE * f64::from(ON[0].ms) / 1000.0) as usize;
        for i in 0..lead {
            let s = player.next_sample().unwrap();
            assert_eq!(s, 0.0, "sample {i} of the lead-in was not silent");
        }
    }
}
