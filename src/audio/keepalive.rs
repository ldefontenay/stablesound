//! The keep-alive render stream.
//!
//! Dropping `KeepAlive` releases the `IAudioClient`, which is what frees the
//! headset for another device. Milestone 1 confirmed on the Soundcore AeroClip
//! that this is sufficient: the iPhone takes over about 3 seconds later. That
//! is the whole reason the app can exist, so the teardown path matters as much
//! as the playback path.

use windows::core::{Result as WinResult, GUID};
use windows::Win32::Foundation::E_NOTIMPL;
use windows::Win32::Media::Audio::{
    IAudioClient, IAudioRenderClient, IMMDevice, AUDCLNT_SHAREMODE_SHARED, WAVEFORMATEX,
    WAVEFORMATEXTENSIBLE,
};
use windows::Win32::System::Com::{CoTaskMemFree, CLSCTX_ALL};

use crate::config::Signal;

/// 100-nanosecond units per second (WASAPI reference time).
const REFTIMES_PER_SEC: i64 = 10_000_000;

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
                REFTIMES_PER_SEC / 2, // 500 ms
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
            })
        }
    }

    /// Top up the shared buffer. Call several times a second.
    pub fn pump(&mut self) -> WinResult<()> {
        unsafe {
            let padding = self.client.GetCurrentPadding()?;
            let available = self.buffer_frames.saturating_sub(padding);
            if available == 0 {
                return Ok(());
            }

            let ptr = self.render.GetBuffer(available)? as *mut f32;
            let blip_every = self.sample_rate as u64;

            for frame in 0..available as u64 {
                let n = self.frames_elapsed + frame;
                let value = match self.signal {
                    Signal::Zeros => 0.0,
                    Signal::Fluctuate => {
                        if n.is_multiple_of(blip_every) {
                            TINY_SAMPLE
                        } else {
                            0.0
                        }
                    }
                    Signal::Sine { freq, amp } => {
                        let phase =
                            (n as f64) * f64::from(freq) * std::f64::consts::TAU / self.sample_rate;
                        (phase.sin() as f32) * amp
                    }
                };
                let base = frame as usize * self.channels;
                for ch in 0..self.channels {
                    *ptr.add(base + ch) = value;
                }
            }

            self.frames_elapsed += available as u64;
            self.render.ReleaseBuffer(available, 0)?;
            Ok(())
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
