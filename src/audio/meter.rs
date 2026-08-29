//! Device-level peak metering, used to tell whether anything real is playing.
//!
//! Reads the peak for the whole endpoint rather than per-session. That is far
//! less code, and it works because the default keep-alive signal is pure
//! silence: our own contribution to the peak is exactly 0.0, so whatever the
//! meter reports is somebody else's audio.
//!
//! Milestone 1 measured JAWS speech at roughly 0.56 against a 0.0005 threshold,
//! so the margin is about 1000x.
//!
//! This only holds while the signal stays quiet. `Config::validate` refuses to
//! pair a loud signal with idle-based release for exactly this reason.

use windows::core::Result as WinResult;
use windows::Win32::Media::Audio::Endpoints::IAudioMeterInformation;
use windows::Win32::Media::Audio::IMMDevice;
use windows::Win32::System::Com::CLSCTX_ALL;

pub struct Meter {
    meter: IAudioMeterInformation,
}

impl Meter {
    pub fn open(device: &IMMDevice) -> WinResult<Self> {
        let meter: IAudioMeterInformation = unsafe { device.Activate(CLSCTX_ALL, None)? };
        Ok(Meter { meter })
    }

    /// Current peak, 0.0 to 1.0. Treats a read failure as silence: a meter that
    /// has gone away is not a reason to keep the headset awake forever.
    pub fn peak(&self) -> f32 {
        unsafe { self.meter.GetPeakValue().unwrap_or(0.0) }
    }
}
