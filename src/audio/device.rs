//! Finding and identifying output devices.
//!
//! Everything here returns or consumes COM objects, which are not `Send`. All
//! of it must be called from the one thread that owns the audio work.

use windows::core::Result as WinResult;
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Media::Audio::{
    eConsole, eRender, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL, STGM_READ};

use crate::config::DeviceSelector;

/// An opened device, with the identifying details we care about.
pub struct OpenDevice {
    pub device: IMMDevice,
    pub id: String,
    pub name: String,
}

/// Name and ID only. Unlike `OpenDevice` this is `Send`, so it can be handed to
/// a settings dialog on another thread.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
}

pub fn enumerator() -> WinResult<IMMDeviceEnumerator> {
    unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
}

/// Every active output device, for presenting a choice to the user.
pub fn list_outputs() -> WinResult<Vec<DeviceInfo>> {
    unsafe {
        let enumerator = enumerator()?;
        let collection = enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)?;
        let count = collection.GetCount()?;
        let mut out = Vec::with_capacity(count as usize);
        for i in 0..count {
            let device = collection.Item(i)?;
            if let (Ok(id), Ok(name)) = (device_id(&device), friendly_name(&device)) {
                out.push(DeviceInfo { id, name });
            }
        }
        Ok(out)
    }
}

/// Open whichever device the config points at.
///
/// A named device that has gone away falls back to the current default rather
/// than failing: headphones get disconnected, and the app should keep working
/// on the laptop speakers instead of dying. The caller logs the substitution.
pub fn open(selector: &DeviceSelector) -> WinResult<(OpenDevice, bool)> {
    unsafe {
        let enumerator = enumerator()?;

        if let DeviceSelector::Named(wanted) = selector {
            let collection = enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)?;
            for i in 0..collection.GetCount()? {
                let device = collection.Item(i)?;
                let name = friendly_name(&device).unwrap_or_default();
                if name.eq_ignore_ascii_case(wanted) {
                    let id = device_id(&device)?;
                    return Ok((OpenDevice { device, id, name }, false));
                }
            }
        }

        let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
        let id = device_id(&device)?;
        let name = friendly_name(&device)?;
        let substituted = matches!(selector, DeviceSelector::Named(_));
        Ok((OpenDevice { device, id, name }, substituted))
    }
}

/// ID of the current default output, for cheap change detection.
///
/// Polled rather than using `IMMNotificationClient`. Implementing a COM
/// callback would be the tidier answer, but polling once a second is far less
/// code for a difference nobody can perceive, and keeps the binary small.
pub fn default_output_id() -> WinResult<String> {
    unsafe {
        let enumerator = enumerator()?;
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
        device_id(&device)
    }
}

pub fn device_id(device: &IMMDevice) -> WinResult<String> {
    unsafe {
        let id = device.GetId()?;
        let text = id.to_string();
        windows::Win32::System::Com::CoTaskMemFree(Some(id.0 as *const _));
        Ok(text?)
    }
}

pub fn friendly_name(device: &IMMDevice) -> WinResult<String> {
    unsafe {
        let store = device.OpenPropertyStore(STGM_READ)?;
        let value = store.GetValue(&PKEY_Device_FriendlyName)?;
        Ok(value.to_string())
    }
}
