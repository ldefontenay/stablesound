//! The keep-alive render stream.
//!
//! Dropping `KeepAlive` releases the `IAudioClient`, which is what frees the
//! headset for another device. Milestone 1 confirmed on the Soundcore AeroClip
//! that this is sufficient: the iPhone takes over about 3 seconds later. That
//! is the whole reason the app can exist, so the teardown path matters as much
//! as the playback path.
//!
//! The stream also carries earcons, because the tone marking a release has to
//! be heard *before* the device is let go - see `earcon`.

use std::time::{Duration, Instant};

use windows::core::{Result as WinResult, GUID};
use windows::Win32::Foundation::E_NOTIMPL;
use windows::Win32::Media::Audio::{
    IAudioClient, IAudioRenderClient, IMMDevice, AUDCLNT_SHAREMODE_SHARED, WAVEFORMATEX,
    WAVEFORMATEXTENSIBLE,
};
use windows::Win32::System::Com::{CoTaskMemFree, CLSCTX_ALL};

use crate::audio::earcon::{Note, Player};
use crate::config::Signal;

/// 100-nanosecond units per second (WASAPI reference time).
const REFTIMES_PER_SEC: i64 = 10_000_000;

/// How much buffer WASAPI allocates for us.
const BUFFER_MS: i64 = 500;

/// How much audio we actually keep queued ahead of the device.
///
/// Deliberately far less than the buffer. Anything already queued has to play
/// out before a newly queued earcon is heard, so filling all 500 ms would put
/// up to half a second between pressing the hotkey and hearing the answer. At
/// 200 ms the feedback is prompt, while 300 ms of unused buffer remains as
/// headroom if the engine thread is ever descheduled.
const TARGET_QUEUE_MS: f64 = 200.0;

const WAVE_FORMAT_IEEE_FLOAT: u16 = 0x0003;
const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;

/// KSDATAFORMAT_SUBTYPE_IEEE_FLOAT
const SUBTYPE_IEEE_FLOAT: GUID = GUID::from_u128(0x00000003_0000_0010_8000_00aa00389b71);

/// Smallest non-zero sample for 16-bit audio, as a float.
const TINY_SAMPLE: f32 = 1.0 / 32768.0;

pub struct KeepAlive {
    client: IAudioClient,
    render: IAudioRenderClient,
    buffer_frames: u32,
    channels: usize,
    sample_rate: f64,
    signal: Signal,
    frames_elapsed: u64,
    /// An earcon being rendered in place of the keep-alive signal.
    earcon: Option<Player>,
}

impl KeepAlive {
    pub fn start(device: &IMMDevice, signal: Signal) -> WinResult<Self> {
        unsafe {
            let client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;
            let format_ptr = client.GetMixFormat()?;
            let format = *format_ptr;

            if !is_float32(format_ptr) {
                CoTaskMemFree(Some(format_ptr as *const _));
                return Err(windows::core::Error::new(
                    E_NOTIMPL,
                    "Device mix format is not 32-bit float; sample conversion is not implemented.",
                ));
            }

            let init = client.Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                0,
                REFTIMES_PER_SEC * BUFFER_MS / 1000,
                0,
                format_ptr,
                None,
            );
            CoTaskMemFree(Some(format_ptr as *const _));
            init?;

            let render: IAudioRenderClient = client.GetService()?;
            let buffer_frames = client.GetBufferSize()?;
            client.Start()?;

            Ok(KeepAlive {
                client,
                render,
                buffer_frames,
                channels: format.nChannels as usize,
                sample_rate: format.nSamplesPerSec as f64,
                signal,
                frames_elapsed: 0,
                earcon: None,
            })
        }
    }

    /// Queue a tone, to be rendered by the next few `pump` calls in place of
    /// the keep-alive signal. Replaces any earcon still playing.
    pub fn play(&mut self, notes: &'static [Note], amplitude: f32) {
        self.earcon = Some(Player::new(notes, self.sample_rate, amplitude));
    }

    /// Top up the queued audio. Call several times a second.
    pub fn pump(&mut self) -> WinResult<()> {
        unsafe {
            let padding = self.client.GetCurrentPadding()?;
            let target = (self.sample_rate * TARGET_QUEUE_MS / 1000.0) as u32;
            let free = self.buffer_frames.saturating_sub(padding);
            let want = target.saturating_sub(padding).min(free);
            if want == 0 {
                return Ok(());
            }

            let ptr = self.render.GetBuffer(want)? as *mut f32;
            let blip_every = self.sample_rate as u64;

            for frame in 0..want as u64 {
                let value = match self.earcon.as_mut().and_then(Player::next_sample) {
                    Some(sample) => sample,
                    None => {
                        // Either there is no earcon, or it just ended. Drop a
                        // finished player so `earcon_playing` stays honest.
                        if self.earcon.as_ref().is_some_and(Player::finished) {
                            self.earcon = None;
                        }
                        self.keepalive_sample(self.frames_elapsed + frame, blip_every)
                    }
                };
                let base = frame as usize * self.channels;
                for ch in 0..self.channels {
                    *ptr.add(base + ch) = value;
                }
            }

            self.frames_elapsed += u64::from(want);
            self.render.ReleaseBuffer(want, 0)?;
            Ok(())
        }
    }

    fn keepalive_sample(&self, n: u64, blip_every: u64) -> f32 {
        match self.signal {
            Signal::Zeros => 0.0,
            Signal::Fluctuate => {
                if n.is_multiple_of(blip_every) {
                    TINY_SAMPLE
                } else {
                    0.0
                }
            }
            Signal::Sine { freq, amp } => {
                let phase = (n as f64) * f64::from(freq) * std::f64::consts::TAU / self.sample_rate;
                (phase.sin() as f32) * amp
            }
        }
    }

    pub fn earcon_playing(&self) -> bool {
        self.earcon.is_some()
    }

    /// Let everything queued reach the headset, then return.
    ///
    /// Called before dropping the stream on a release, so the off-tone is
    /// actually heard. Without it, `Drop` calls `Stop` and `Reset`, which
    /// discard the buffer and swallow the tone.
    ///
    /// Bounded by `timeout`: this blocks the engine thread, and a device that
    /// has stopped consuming must not be able to wedge it.
    pub fn drain(&mut self, timeout: Duration) {
        let deadline = Instant::now() + timeout;

        // First, write out whatever is left of the earcon.
        while self.earcon_playing() && Instant::now() < deadline {
            if self.pump().is_err() {
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }

        // Then stop writing and wait for the queue to empty. Padding only
        // falls now that nothing is topping it up.
        let nearly_empty = (self.sample_rate * 0.005) as u32;
        while Instant::now() < deadline {
            match unsafe { self.client.GetCurrentPadding() } {
                Ok(padding) if padding <= nearly_empty => break,
                Ok(_) => std::thread::sleep(Duration::from_millis(5)),
                Err(_) => break,
            }
        }
    }
}

impl Drop for KeepAlive {
    fn drop(&mut self) {
        // Stop before the interfaces release, so the endpoint goes idle
        // promptly rather than whenever the last reference happens to drop.
        unsafe {
            let _ = self.client.Stop();
            let _ = self.client.Reset();
        }
    }
}

unsafe fn is_float32(format: *const WAVEFORMATEX) -> bool {
    let f = &*format;
    if f.wBitsPerSample != 32 {
        return false;
    }
    match f.wFormatTag {
        WAVE_FORMAT_IEEE_FLOAT => true,
        WAVE_FORMAT_EXTENSIBLE => {
            // WAVEFORMATEXTENSIBLE is packed, so the field cannot be borrowed.
            let ext = format as *const WAVEFORMATEXTENSIBLE;
            std::ptr::addr_of!((*ext).SubFormat).read_unaligned() == SUBTYPE_IEEE_FLOAT
        }
        _ => false,
    }
}
