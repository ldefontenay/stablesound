//! StableSound Milestone 1 spike.
//!
//! Throwaway diagnostic tool. Its only job is to answer the untested assumptions
//! in PLAN.md section 6 before we write the real app:
//!
//!   Q1  Does closing the audio stream actually free the AeroClip for the iPhone?
//!   Q2  Does JAWS speech register on the device-level peak meter?
//!   Q3  Which keep-alive signal does the AeroClip actually need?
//!
//! Line-based commands (type, then Enter) rather than raw keypresses, and output
//! only on state transitions rather than a live meter, so it is usable with a
//! screen reader.

use std::io::{self, BufRead, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use windows::core::{Result as WinResult, GUID};
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Media::Audio::Endpoints::IAudioMeterInformation;
use windows::Win32::Media::Audio::{
    eConsole, eRender, IAudioClient, IAudioRenderClient, IMMDevice, IMMDeviceEnumerator,
    MMDeviceEnumerator, AUDCLNT_SHAREMODE_SHARED, WAVEFORMATEX, WAVEFORMATEXTENSIBLE,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED, STGM_READ,
};

/// 100-nanosecond units in one second (WASAPI reference time).
const REFTIMES_PER_SEC: i64 = 10_000_000;

const WAVE_FORMAT_IEEE_FLOAT: u16 = 0x0003;
const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;

/// KSDATAFORMAT_SUBTYPE_IEEE_FLOAT
const SUBTYPE_IEEE_FLOAT: GUID = GUID::from_u128(0x00000003_0000_0010_8000_00aa00389b71);

/// Smallest non-zero sample for 16-bit audio, expressed as a float. This is the
/// value Sound Keeper's "fluctuate" mode uses: inaudible, but not a pure zero
/// that the Windows audio engine might optimise away.
const TINY_SAMPLE: f32 = 1.0 / 32768.0;

/// Peak level above which we call it "real audio". Comfortably above the
/// fluctuate signal (~0.00003) but well below anything audible.
const AUDIO_THRESHOLD: f32 = 0.0005;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Mode {
    /// Pure digital silence. May not work: Windows can optimise it away.
    Zeros,
    /// Zeros plus one tiny non-zero sample per second. Sound Keeper's default.
    Fluctuate,
    /// 20 kHz sine at 1%. Inaudible to humans. What the NVDA add-on uses.
    Sine,
}

impl Mode {
    fn label(self) -> &'static str {
        match self {
            Mode::Zeros => "zeros (pure silence)",
            Mode::Fluctuate => "fluctuate (silence + tiny blip each second)",
            Mode::Sine => "sine (20 kHz at 1%, inaudible)",
        }
    }
}

enum Cmd {
    On(Mode),
    Off,
    Quit,
}

fn main() {
    println!("StableSound spike - Milestone 1 diagnostics");
    println!();

    match describe_default_device() {
        Ok(name) => println!("Default output device: {name}"),
        Err(e) => {
            eprintln!("Could not read the default output device: {e}");
            eprintln!("Is anything connected and set as default?");
            return;
        }
    }

    println!();
    print_help();

    let (tx, rx) = mpsc::channel::<Cmd>();
    let audio = thread::spawn(move || {
        if let Err(e) = audio_thread(rx) {
            eprintln!("\nAudio thread failed: {e}");
        }
    });

    let watching = Arc::new(AtomicBool::new(false));
    let running = Arc::new(AtomicBool::new(true));
    let meter = {
        let watching = Arc::clone(&watching);
        let running = Arc::clone(&running);
        thread::spawn(move || {
            if let Err(e) = meter_thread(watching, running) {
                eprintln!("\nMeter thread failed: {e}");
            }
        })
    };

    let mut mode = Mode::Fluctuate;
    let mut on = false;

    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();

    loop {
        print!("> ");
        let _ = io::stdout().flush();

        let Some(Ok(line)) = lines.next() else { break };
        let cmd = line.trim().to_lowercase();

        match cmd.as_str() {
            "" => continue,
            "on" => {
                let _ = tx.send(Cmd::On(mode));
                on = true;
                println!("KEEP-ALIVE ON - {}", mode.label());
                if mode == Mode::Sine && watching.load(Ordering::Relaxed) {
                    println!(
                        "  NOTE: sine mode is loud enough to trip the peak meter, so the watcher \
                         will see our own signal as 'real audio'. That is itself a finding - see \
                         the note at the end of this session."
                    );
                }
            }
            "off" => {
                let _ = tx.send(Cmd::Off);
                on = false;
                println!("KEEP-ALIVE OFF - stream closed and device released");
            }
            "zeros" | "fluct" | "fluctuate" | "sine" => {
                mode = match cmd.as_str() {
                    "zeros" => Mode::Zeros,
                    "sine" => Mode::Sine,
                    _ => Mode::Fluctuate,
                };
                println!("Mode set to {}", mode.label());
                if on {
                    let _ = tx.send(Cmd::On(mode));
                    println!("Restarted with the new mode.");
                }
            }
            "watch" => {
                let now = !watching.load(Ordering::Relaxed);
                watching.store(now, Ordering::Relaxed);
                if now {
                    println!("Watching for audio. Speak with JAWS to test.");
                } else {
                    println!("Stopped watching.");
                }
            }
            "status" => {
                println!(
                    "Keep-alive: {}   Mode: {}   Watching: {}",
                    if on { "ON" } else { "off" },
                    mode.label(),
                    if watching.load(Ordering::Relaxed) {
                        "yes"
                    } else {
                        "no"
                    }
                );
            }
            "help" | "?" => print_help(),
            "quit" | "exit" | "q" => break,
            other => println!("Unknown command: {other}. Type help for the list."),
        }
    }

    let _ = tx.send(Cmd::Quit);
    running.store(false, Ordering::Relaxed);
    let _ = audio.join();
    let _ = meter.join();
    println!("Done.");
}

fn print_help() {
    println!("Commands (type, then press Enter):");
    println!("  on        start keep-alive");
    println!("  off       stop keep-alive and fully release the device");
    println!("  zeros     use pure silence");
    println!("  fluct     use silence with a tiny blip each second (default)");
    println!("  sine      use an inaudible 20 kHz tone");
    println!("  watch     toggle reporting when other audio starts and stops");
    println!("  status    show current state");
    println!("  quit      exit");
    println!();
    println!("Suggested test run:");
    println!("  Q3  'on', then leave it a few minutes and check JAWS speech is not clipped.");
    println!("      If it still clips, try another mode.");
    println!("  Q1  With keep-alive ON, play something on the iPhone. Expect it NOT to take over.");
    println!("      Then 'off' and try the phone again. Note how long the handover takes.");
    println!("  Q2  'watch', then let JAWS speak. It should report audio starting and stopping.");
}

// ---------------------------------------------------------------------------
// Audio
// ---------------------------------------------------------------------------

/// Owns the WASAPI render stream. Dropping this releases the device, which is
/// exactly the "soft release" behaviour Q1 is testing.
struct Engine {
    client: IAudioClient,
    render: IAudioRenderClient,
    buffer_frames: u32,
    channels: usize,
    sample_rate: f64,
    mode: Mode,
    /// Frames emitted since start, for the fluctuate blip and the sine phase.
    frames_elapsed: u64,
}

impl Engine {
    fn start(mode: Mode) -> WinResult<Self> {
        unsafe {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let device: IMMDevice = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
            let client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;

            let format_ptr = client.GetMixFormat()?;
            let format = *format_ptr;

            if !is_float32(format_ptr) {
                CoTaskMemFree(Some(format_ptr as *const _));
                return Err(windows::core::Error::new(
                    windows::Win32::Foundation::E_NOTIMPL,
                    "Device mix format is not 32-bit float. The spike only handles float32; \
                     the real app will need to convert.",
                ));
            }

            let result = client.Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                0,
                REFTIMES_PER_SEC / 2, // 500 ms buffer
                0,
                format_ptr,
                None,
            );
            CoTaskMemFree(Some(format_ptr as *const _));
            result?;

            let render: IAudioRenderClient = client.GetService()?;
            let buffer_frames = client.GetBufferSize()?;
            client.Start()?;

            Ok(Engine {
                client,
                render,
                buffer_frames,
                channels: format.nChannels as usize,
                sample_rate: format.nSamplesPerSec as f64,
                mode,
                frames_elapsed: 0,
            })
        }
    }

    /// Top up the shared buffer. Called regularly from the audio thread.
    fn pump(&mut self) -> WinResult<()> {
        unsafe {
            let padding = self.client.GetCurrentPadding()?;
            let available = self.buffer_frames.saturating_sub(padding);
            if available == 0 {
                return Ok(());
            }

            let ptr = self.render.GetBuffer(available)? as *mut f32;
            let blip_every = self.sample_rate as u64; // one blip per second

            for frame in 0..available as u64 {
                let n = self.frames_elapsed + frame;
                let value = match self.mode {
                    Mode::Zeros => 0.0,
                    Mode::Fluctuate => {
                        if n % blip_every == 0 {
                            TINY_SAMPLE
                        } else {
                            0.0
                        }
                    }
                    Mode::Sine => {
                        let phase =
                            (n as f64) * 20_000.0 * std::f64::consts::TAU / self.sample_rate;
                        (phase.sin() * 0.01) as f32
                    }
                };
                for ch in 0..self.channels {
                    *ptr.add(frame as usize * self.channels + ch) = value;
                }
            }

            self.frames_elapsed += available as u64;
            self.render.ReleaseBuffer(available, 0)?;
            Ok(())
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        unsafe {
            let _ = self.client.Stop();
        }
    }
}

fn audio_thread(rx: Receiver<Cmd>) -> WinResult<()> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }

    let mut engine: Option<Engine> = None;

    loop {
        match rx.try_recv() {
            Ok(Cmd::On(mode)) => {
                engine = None; // release any existing stream first
                match Engine::start(mode) {
                    Ok(e) => engine = Some(e),
                    Err(err) => eprintln!("\nCould not start keep-alive: {err}"),
                }
            }
            Ok(Cmd::Off) => engine = None,
            Ok(Cmd::Quit) | Err(TryRecvError::Disconnected) => break,
            Err(TryRecvError::Empty) => {}
        }

        if let Some(e) = engine.as_mut() {
            if let Err(err) = e.pump() {
                eprintln!("\nKeep-alive stopped: {err}");
                engine = None;
            }
        }

        thread::sleep(Duration::from_millis(50));
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Peak meter
// ---------------------------------------------------------------------------

/// Polls the device peak meter and reports only on transitions, so it stays
/// readable with a screen reader.
fn meter_thread(watching: Arc<AtomicBool>, running: Arc<AtomicBool>) -> WinResult<()> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let device: IMMDevice = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
        let meter: IAudioMeterInformation = device.Activate(CLSCTX_ALL, None)?;

        let mut audio_on = false;
        let mut since = Instant::now();
        let mut peak_seen: f32 = 0.0;

        while running.load(Ordering::Relaxed) {
            if watching.load(Ordering::Relaxed) {
                let peak = meter.GetPeakValue().unwrap_or(0.0);
                let loud = peak > AUDIO_THRESHOLD;

                if loud && !audio_on {
                    audio_on = true;
                    since = Instant::now();
                    peak_seen = peak;
                    println!("\n  AUDIO DETECTED (peak {peak:.5})");
                    print!("> ");
                    let _ = io::stdout().flush();
                } else if loud {
                    peak_seen = peak_seen.max(peak);
                } else if !loud && audio_on {
                    audio_on = false;
                    println!(
                        "\n  AUDIO STOPPED after {:.1} s (highest peak {peak_seen:.5})",
                        since.elapsed().as_secs_f32()
                    );
                    print!("> ");
                    let _ = io::stdout().flush();
                    peak_seen = 0.0;
                }
            }
            thread::sleep(Duration::from_millis(100));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

unsafe fn is_float32(format: *const WAVEFORMATEX) -> bool {
    let f = &*format;
    if f.wBitsPerSample != 32 {
        return false;
    }
    match f.wFormatTag {
        WAVE_FORMAT_IEEE_FLOAT => true,
        WAVE_FORMAT_EXTENSIBLE => {
            let ext = format as *const WAVEFORMATEXTENSIBLE;
            // WAVEFORMATEXTENSIBLE is packed, so this field cannot be referenced
            // directly; read it unaligned.
            let subformat = std::ptr::addr_of!((*ext).SubFormat).read_unaligned();
            subformat == SUBTYPE_IEEE_FLOAT
        }
        _ => false,
    }
}

fn describe_default_device() -> WinResult<String> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let device: IMMDevice = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
        let store = device.OpenPropertyStore(STGM_READ)?;
        let value = store.GetValue(&PKEY_Device_FriendlyName)?;
        Ok(value.to_string())
    }
}
