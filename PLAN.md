# StableSound — research findings and build plan

**Date:** 2026-08-29
**Goal:** Stop JAWS speech cutting out on Bluetooth headphones, without permanently monopolising the headset, so the iPhone can still take over.

---

## 1. The problem

Modern Bluetooth headphones power down their audio receiver after a few seconds with no incoming audio, to save battery. When JAWS next speaks, the first fraction of a second (sometimes more) is lost while the headset wakes up. The standard fix is to continuously stream inaudible audio so the headset never idles.

The catch: the headset is paired to both the laptop and an iPhone (multipoint). While the laptop is streaming, the headset stays bound to the laptop and the phone cannot take over. So keep-alive must be something that can be switched **off** on demand, and that switches itself off when not needed.

**Hardware (confirmed):** Anker **Soundcore AeroClip** open-ear clip buds.

---

## 2. Research findings

### 2.1 JAWS' built-in feature

JAWS has this natively. It lives in Settings Center:

- `Insert+6` to open Settings Center, `Ctrl+Shift+D` for Default settings
- Search for "blue" or "avoid"
- Setting: **"Avoid speech cut-off when using Bluetooth headphones or some cards"**

It works, but it is all-or-nothing: on means always on. There is no hotkey and no auto-release, which is exactly the limitation we are solving. Freedom Scientific's own docs note it will drain headphone battery faster.

**Not verified:** whether a JAWS script can toggle this setting at runtime and have it take effect without restarting JAWS. Settings Center options vary — some apply live, some need a restart — and the config key name for this one is not documented publicly. Rather than gamble on it, the plan below sidesteps JAWS entirely: **turn the JAWS setting off permanently and let our app own the behaviour.** That also means the fix works for system audio and any other screen reader, not just JAWS.

### 2.2 Existing tools

| Tool | What it does | Why it doesn't fit |
|---|---|---|
| **Silenzio** (Štefan Kiss, v2.1, May 2026) | Tray app, plays a silent WAV, picks output device, remembers last device | Closed source, installer-only (no portable build), no hotkey, no auto-release. Can't build on it. |
| **Sound Keeper** (vrubleg, v1.3.7, June 2026) | C++, MIT, actively maintained. Device selection (primary/all/digital/analog/marked), five stream modes, `soundkeeper kill` to stop | No GUI, no hotkey, no auto-release timer. Best *reference implementation* we have. |
| **keep-audio-alive** (TarnishedStella) | MIT, Electron + React. Device selection, idle detection, tray, autostart | Electron (~100 MB+), no global hotkey, accessibility with JAWS unverified. Wrong shape. |
| **SleepSoundly** (trypsynth) | Tiny C, MIT, tray icon, handles device changes | No hotkey, no timer, minimal and not actively developed. |
| **silent_bt_awake** (nunuvin) | Plays silence for the Windows 11 Bluetooth delay | Bare script, none of the control we need. |
| **NVDA Bluetooth Audio** (mltony) | NVDA add-on, plays inaudible 20 kHz sine | NVDA-only, explicitly has **no** keystrokes and no options. |

**Conclusion: nothing existing has hotkey toggle + auto-release.** Every tool found is "on until you quit it". We are building something genuinely absent, not reinventing. Sound Keeper is MIT-licensed C++ and is the right thing to read for the audio internals.

### 2.3 The one non-obvious technical fact

**Streaming pure digital zeros may not work.** Windows' audio engine can detect an all-silent buffer and optimise it away, letting the endpoint sleep anyway. This is why Sound Keeper's *default* mode is called "fluctuate" — zeros with the smallest possible non-zero sample injected once per second — and why it also offers an inaudible sine option. The NVDA add-on takes the other route: a 20 kHz sine at low volume, above the range of human hearing.

Design consequence: make the keep-alive signal a **configurable strategy** (fluctuate / inaudible sine / pure zeros) rather than hardcoding one. Different headsets respond differently and we may need to experiment on the AeroClip specifically.

### 2.4 Multipoint behaviour

Soundcore confirms dual connection (multipoint) is **on by default** on AeroClip and can be managed in the Soundcore app. Confirmed: two devices connected at once, switching without re-pairing.

**Not verified, and this is the critical unknown:** what exactly the AeroClip needs from the laptop before it will let the phone take over. Specifically, whether merely stopping our audio stream is enough, or whether Windows keeps the A2DP link "in use" such that the phone stays locked out. This is the single assumption the whole design rests on, so it gets tested first — see Milestone 1.

---

## 3. Decisions

| Decision | Choice | Why |
|---|---|---|
| Release trigger | **Idle-based**, with a fixed-timer mode as an option | Keep-alive should follow actual use. Any real audio resets the countdown, so it stays alive while you work and releases once you genuinely stop. Fixed timer available for a hard cutoff. |
| Release depth | **Soft release** default, **hard release** optional | Soft = close the audio stream, let Windows go idle. Hard = disconnect the headset's audio profile from Windows outright. Hard is more certain but costs a reconnect delay, so it's opt-in. |
| Language | **Rust** | ~1 MB single portable exe, no runtime, no installer. Easy to hand to other people. Direct WASAPI access, which suits an app that is fundamentally about audio stream lifetime. |
| GUI | **Real Win32 controls via `winsafe`** | Non-negotiable for a screen reader tool: real `HWND` controls are accessible to JAWS for free. Drawn-UI toolkits (egui, iced, Slint) reconstruct an accessibility tree via AccessKit — works, but consistently worse under JAWS. **Note:** `native-windows-gui`, the crate most tutorials recommend, is no longer maintained. `winsafe` (v0.0.28, July 2026) is the live alternative. |
| Toolchain | rustup + MSVC (Visual Studio Build Tools) | The GNU toolchain avoids the download but adds COM/linking friction. Not worth it. |
| Feedback | **Earcons** (distinct short tones for on/off) | Played through the target device, so hearing the tone also proves the headset is awake. Optional speech can layer on later. |

---

## 4. Design

### 4.1 States

```
RELEASED       --hotkey / audio detected-------->  KEEPING_ALIVE
KEEPING_ALIVE  --hotkey / idle timeout expires-->  RELEASED
```

- **KEEPING_ALIVE** — WASAPI render stream open on the target device, emitting the configured keep-alive signal.
- **RELEASED** — stream fully closed (`IAudioClient` released, not merely paused — a paused stream may still hold the endpoint). Optionally, Bluetooth audio profile disconnected.

### 4.2 Idle detection

Poll the **device-level peak meter** (`IAudioMeterInformation` on the `IMMDevice`) at around 10 Hz.

This is simpler than enumerating individual audio sessions, and it works because our own keep-alive signal is vanishingly quiet — fluctuate mode peaks around 0.00003, far below any sensible threshold. So the device peak effectively reports *other* apps' audio, including JAWS speech.

- Peak above threshold → reset the idle countdown
- Countdown reaches zero → release

**Fallback if that proves unreliable:** enumerate sessions via `IAudioSessionManager2`, skip our own process ID, and check each session's state and peak. More code, more precise. Only go here if needed.

**To verify:** that JAWS speech actually registers on the device peak meter. JAWS may use an audio path that behaves differently. Tested in Milestone 1.

**Known interaction, found while building the spike:** device-level metering only works if our own keep-alive signal stays below the detection threshold. Fluctuate (~0.00003) and zeros are fine against a 0.0005 threshold. **Sine at 1% (0.01) is not** — we would see our own signal as "real audio" and never release. So if Q3 concludes the AeroClip needs sine, idle detection *must* move to the per-session fallback. The two choices are coupled.

### 4.3 Settings

- Target device: default output, or a specific device (remembered by name, surviving re-plugging)
- Keep-alive signal: fluctuate / inaudible sine (frequency + amplitude) / pure zeros
- Release mode: idle-based (default) or fixed timer
- Idle timeout: seconds/minutes, default around 60s — needs tuning against real use
- Fixed timer duration
- Hard release: on/off
- Global hotkey: user-assignable
- Start with Windows: on/off
- Earcons: on/off, volume

Stored as a small config file next to the exe if that location is writable, otherwise in `%APPDATA%` — this keeps the portable build genuinely portable.

### 4.4 Accessibility rules

These are requirements, not nice-to-haves:

1. Every function reachable by keyboard without the tray menu. Tray icons are awkward with a screen reader; the global hotkey is the primary interface, plus a second hotkey to open settings.
2. Real Win32 controls only.
3. Every control has a proper label and a sensible tab order.
4. The settings dialog is a real dialog — Escape cancels, Enter confirms, standard behaviour.
5. State changes are always audible (earcons), never visual-only.

---

## 5. Stack

- **Rust**, stable-msvc toolchain
- **`winsafe`** — native Win32 controls, settings dialog. Pinned to an exact version (it's 0.0.x and the API churns).
- **`windows-sys`** or **`windows`** — WASAPI (`IAudioClient`, `IAudioRenderClient`, `IMMDeviceEnumerator`, `IAudioMeterInformation`), plus `RegisterHotKey` and `Shell_NotifyIcon`
- **`embed-manifest`** — build dependency, for the app manifest
- Release profile tuned for size: `opt-level = "z"`, LTO, `panic = "abort"`, strip symbols

Target: single self-contained `.exe`, roughly 300 KB – 1 MB, no installer.

---

## 6. Open questions

Ordered by how much damage a wrong answer does.

1. **Does soft release actually free the headset for the iPhone?** The core assumption. If Windows holds the A2DP link "in use" regardless, hard release stops being optional and becomes the default. Test before building anything else.
2. **Does JAWS speech show on the device peak meter?** If not, idle detection needs the per-session fallback.
3. **Which keep-alive signal does the AeroClip actually need?** Fluctuate, inaudible sine, or is pure zeros enough? Empirical, per device.
4. **What idle timeout feels right?** Too short and speech cuts out mid-task; too long and the phone stays locked out. Needs real-world use to tune. Start at 60s.
5. **How long does the AeroClip take to reconnect after a hard release?** Determines whether hard release is usable day-to-day or a last resort.

---

## 7. Plan

**Milestone 0 — toolchain. DONE (2026-08-29).** rustup + Rust 1.98.0, Visual Studio Build Tools 17.14.39 with the VC++ workload. Verified end to end: a real binary compiles, links and runs.

**Milestone 1 — spike. BUILT, awaiting hardware test.** `spike/` is a throwaway console tool that opens a WASAPI stream, emits a selectable keep-alive signal, and fully releases the device on command. Line-based commands and transition-only output, so it is usable with a screen reader.

Run with `cargo run` from `spike/`. Commands: `on`, `off`, `zeros`, `fluct`, `sine`, `watch`, `status`, `quit`.

Connect the AeroClip and make it the default output device first — the spike targets the default endpoint and reports which one it picked at startup.

**Milestone 2 — core engine.** Keep-alive strategies, device selection and hot-plug handling, the state machine, device peak-meter idle detection, clean stream teardown.

**Milestone 3 — control surface.** Global hotkey registration, earcons for on/off, tray icon for sighted users and for a visible quit.

**Milestone 4 — settings.** The `winsafe` dialog, config load/save, second hotkey to open settings. Test the whole dialog under JAWS with the screen off.

**Milestone 5 — hard release.** Bluetooth audio profile disconnect/reconnect as an opt-in mode. Reference: `m2jean/ToothTray` does exactly this and is open source.

**Milestone 6 — ship.** Autostart, size-tuned release build, README, and a plan for the antivirus/SmartScreen problem — a small unsigned binary that opens audio devices and registers global hotkeys fits the profile AV heuristics dislike. Options: submit false-positive reports to the major vendors, or look at code signing.

**First action after this document:** Milestone 0, then the Milestone 1 spike.

---

## 8. Sources

**JAWS**
- [A.T. Guys — How to stop Bluetooth Audio cutting off](https://support.atguys.com/hc/en-us/articles/4405331914132-How-to-stop-Bluetooth-Audio-from-cutting-off-on-my-computer)
- [jfw.groups.io — JAWS 2020 feature for stopping speech cut off, FSCast 186](https://jfw.groups.io/g/main/message/80810)
- [jfw.groups.io — Sound Card Keep Awake Option](https://jfw.groups.io/g/main/message/80375)
- [Freedom Scientific — Changing settings in JAWS](https://doccenter.freedomscientific.com/doccenter/doccenter/rs11f929e9c511/2015-02-10_teacherstrainers-l3/02_ChangingSettingsInJAWS.htm)

**Existing tools**
- [Sound Keeper — GitHub](https://github.com/vrubleg/soundkeeper) · [ReadMe](https://github.com/vrubleg/soundkeeper/blob/master/ReadMe.txt) · [project page](https://veg.by/en/projects/soundkeeper/)
- [Silenzio on the Nathan Tech archive](https://software.nathantech.net/archive.php?method=Silenzio) · [jfw.groups.io — Silenzio 2.0 is Out](https://jfw.groups.io/g/main/topic/silenzio_2_0_is_out/81801660) · [author's site](https://www.stefankiss.sk/)
- [keep-audio-alive](https://github.com/TarnishedStella/keep-audio-alive)
- [SleepSoundly](https://github.com/trypsynth/SleepSoundly/)
- [silent_bt_awake](https://github.com/nunuvin/silent_bt_awake)
- [NVDA Bluetooth Audio add-on](https://github.com/mltony/nvda-bluetooth-audio)
- [ToothTray — Bluetooth connect/disconnect](https://github.com/m2jean/ToothTray)

**Background**
- [Jonathan Mosen — Sounds frustrating!](https://mosen.org/soundsfrustrating/)
- [SilenceTool — playing silence to keep Bose 700 awake](https://medium.com/@chiaracoetzee/silencetool-playing-silence-in-c-to-keep-my-bose-700-headphones-awake-aef3c63c88c5)
- [Microsoft Q&A — preventing Windows suspending audio after inactivity](https://learn.microsoft.com/en-us/answers/questions/f6ecaaf7-567b-4d92-9c28-4a394160ac6c/how-to-prevent-windows-from-suspending-audio-after)

**Rust**
- [winsafe — GitHub](https://github.com/rodrigocfd/winsafe) · [lib.rs](https://lib.rs/crates/winsafe) · [gui docs](https://docs.rs/winsafe/latest/winsafe/gui/)
- [native-windows-gui — no longer maintained](https://lib.rs/crates/native-windows-gui)
- [Programming native and accessible Windows GUIs with Rust](https://modulus.isonomia.net/tech/nwg/)
- [AccessKit](https://accesskit.dev/)

**Hardware**
- [Soundcore AeroClip FAQ](https://service.soundcore.com/article-description/soundcore-AeroClip-FAQ)
- [Soundcore — How to Use Multipoint Connection](https://service.soundcore.com/article-description/How-to-Use-Multipoint-Connection)
