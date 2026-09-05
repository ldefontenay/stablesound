# StableSound - research findings and build plan

## 0. Where we are (updated 2026-09-05)

**Read this first when picking the project up.**

Milestones 0, 1 and 2 are complete, hardware round included. Milestone 3 is
under way.

### What exists and works

A working console app. `cargo run` from the repo root. It keeps the headphones
awake, releases them on a timer, wakes again when you touch the keyboard, and
logs everything to `stablesound.log` next to the exe.

Release binary is **248 KB**, against a ~1 MB budget.

### What is proven on hardware

- Releasing the audio stream frees the AeroClip for the iPhone in ~3 seconds.
- Pure digital silence is enough to stop speech clipping. No tone needed.
- JAWS speech reads ~0.56 on the device peak meter; the threshold is 0.0005.
- Speech correctly postpones release; quiet correctly triggers it.
- The log is readable and complete (the user's words: "Yes, clear").
- Device-loss recovery works. Disconnecting the AeroClip and reconnecting it
  ten seconds later brought keep-alive back on its own - no typing, no stuck
  state, no message spam. The Milestone 2 Test 5 failure is closed.
- Waking on input holds up in real use, and a manual switch-off correctly
  stays off, so the iPhone can still take the headset.
- 30 s beats 60 s as the idle timeout.

### What the hardware round asked for

Two changes came out of Tests 8-10, both from the tester, both now design:

- **Do not wake on the mouse.** The tester works without a mouse and can brush
  the trackpad by accident, which grabs the headset back. Waking stays on
  keyboard input; mouse waking becomes an option, off by default. See 4.1.
- **Make the timeout easy to change.** Editing a config file by hand does not
  count as easy. This is a requirement for the Milestone 4 dialog, not a
  nice-to-have.

### Known residual risks, still untested

- Whether zeros still works after a much longer idle gap - an hour away from
  the desk rather than 30 seconds (question 6).
- Behaviour across Windows sleep/resume, and on battery (question 7).

### What is next

**Milestone 3 is built and needs a hardware round.** Hotkey, earcons and tray
all work, and none of them has been near the AeroClip. `test.txt` is the script;
Tests 11-18. The things most likely to come back wrong are matters of taste
rather than correctness - earcon volume and length, and whether hearing a tone
every time keep-alive re-arms itself becomes a nuisance.

The console harness is **kept alongside** the tray, on its own stdin thread,
rather than replaced as originally planned. It is the only diagnostic interface
that exists, the tester is already fluent in it, and throwing it away before the
GUI is proven on hardware would leave nothing to fall back on. Milestone 6 drops
it and switches to the windows subsystem.

After the round: **Milestone 4, the settings dialog** - which carries the
tester's request that the timeout be easy to change.

### Things a fresh session should not re-litigate

- Rust with `winsafe` for native Win32 controls. Not a drawn-UI toolkit; JAWS is
  the whole point. `native-windows-gui` is unmaintained - do not use it.
- Hard release (disconnecting Bluetooth) is **not** being built. Soft release
  was proven sufficient.
- Keep-alive never starts because audio was detected. Only explicit request or
  user input. Section 4.1 explains why.
- Zeros is the default signal. Sine is a last resort and forces fixed release.

---

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

**RESOLVED ON HARDWARE (2026-08-30).** All three modes prevented clipping on the AeroClip, including **pure zeros**. The test discriminated properly - with keep-alive off, speech clipped after 2-3 seconds, so the modes were genuinely doing the work.

This contradicts the received wisdom above, and it is good news. On this hardware and Windows build, what keeps the link alive appears to be the render stream being *open and running*, not the content of the samples. So **zeros becomes the default**: it is the quietest signal possible, which gives idle detection unlimited headroom (see 4.2).

Fluctuate and sine stay implemented as fallbacks for other headsets that may need them - the research says some do - but the AeroClip does not. Sine is demoted furthest: the tester noted a possible faint artifact with it once, and 20 kHz sits close to Nyquist at a 48 kHz sample rate, so aliasing is plausible. Not a good default.

### 2.4 Multipoint behaviour

Soundcore confirms dual connection (multipoint) is **on by default** on AeroClip and can be managed in the Soundcore app. Confirmed: two devices connected at once, switching without re-pairing.

**RESOLVED ON HARDWARE (2026-08-30). The core assumption holds.** Tested twice, same result both times:

- With keep-alive running, the iPhone **could not** take over. The laptop kept control.
- After releasing the stream, the iPhone took over in **about 3 seconds**.

So soft release is sufficient. Closing the WASAPI client frees the headset for the phone, with a handover fast enough to feel immediate. This was the single biggest risk in the design and it is now retired.

**Consequence: hard release is probably unnecessary.** Milestone 5 drops from "planned" to "only if a real need appears" - for instance if another headset behaves differently, or if the 3-second handover proves annoying in practice. Not worth building on spec.

---

## 3. Decisions

| Decision | Choice | Why |
|---|---|---|
| Release trigger | **Idle-based**, with a fixed-timer mode as an option | Keep-alive should follow actual use. Any real audio resets the countdown, so it stays alive while you work and releases once you genuinely stop. Fixed timer available for a hard cutoff. |
| Release depth | **Soft release only** (revised 2026-08-30) | Hardware testing confirmed soft release frees the headset in ~3 s. Hard release was going to be the fallback if it didn't; it isn't needed. Deferred indefinitely rather than built on spec. |
| Keep-alive signal | **Zeros** by default (decided 2026-08-30) | Pure silence proved sufficient on the AeroClip. Quietest possible signal, so it cannot interfere with idle detection. Fluctuate and sine remain available for other hardware. |
| Language | **Rust** | ~1 MB single portable exe, no runtime, no installer. Easy to hand to other people. Direct WASAPI access, which suits an app that is fundamentally about audio stream lifetime. |
| GUI | **Real Win32 controls via `winsafe`** | Non-negotiable for a screen reader tool: real `HWND` controls are accessible to JAWS for free. Drawn-UI toolkits (egui, iced, Slint) reconstruct an accessibility tree via AccessKit — works, but consistently worse under JAWS. **Note:** `native-windows-gui`, the crate most tutorials recommend, is no longer maintained. `winsafe` (v0.0.28, July 2026) is the live alternative. |
| Toolchain | rustup + MSVC (Visual Studio Build Tools) | The GNU toolchain avoids the download but adds COM/linking friction. Not worth it. |
| Wake on input | **Keyboard by default, mouse optional and off** (revised 2026-09-05) | Requested after Milestone 2 testing. Once idle release has fired, the next sound is often a notification, and its first word gets clipped. Touching a key is a reliable sign speech is about to be wanted. Switching off by hand disarms it, so releasing for the phone still sticks. The hardware round then asked for the mouse to be excluded: the tester does not use one and can brush the trackpad by accident, which takes the headset back off the phone. |
| Idle timeout | **30 s** (revised 2026-08-30) | 60 s felt too long in use once waking on input made re-arming cheap. |
| Feedback | **Earcons** (distinct short tones for on/off) | Played through the target device, so hearing the tone also proves the headset is awake. Optional speech can layer on later. |

---

## 4. Design

### 4.1 States

```
RELEASED       --hotkey--------------------------->  KEEPING_ALIVE
RELEASED       --keyboard input, if armed--------->  KEEPING_ALIVE
KEEPING_ALIVE  --hotkey / timeout expires-------->  RELEASED
```

"Armed" means user input may start keep-alive. Switching off **by hand** clears
it; an **automatic** release leaves it set. So deliberately handing the headset
to the phone is not undone by the next keypress, while merely pausing for a
while is.

**Corrected during Milestone 2.** The original diagram had "audio detected" as a
second way into KEEPING_ALIVE. That was wrong, and would have defeated the app's
main purpose: after releasing the headset so the iPhone can take over, the next
word JAWS spoke would have grabbed it straight back, roughly a second later.

**Audio never starts keep-alive; it only postpones release.**

**User input is different in kind, and does start it** (added after Milestone 2
testing, at the tester's suggestion). Keyboard and mouse activity is not caused
by our own output, so it cannot form the feedback loop that makes audio-waking
useless. It also closes a real gap: after an automatic release, the next thing
to make a sound is often a notification, and that first word would be clipped.

Input is detected with `GetLastInputInfo`, deliberately not a `WH_KEYBOARD_LL`
hook. A small unsigned binary that reads every keystroke is exactly the shape
antivirus heuristics flag, which this project already expects to fight at
Milestone 6. `GetLastInputInfo` returns only a timestamp, never key data.

**Keyboard versus mouse (added 2026-09-05).** The hardware round asked for the
mouse to stop waking keep-alive: the tester does not use one, and brushing the
trackpad by accident takes the headset back off the phone. `GetLastInputInfo`
reports a single timestamp for all input and cannot say what caused it, so the
source is inferred by pairing it with `GetCursorPos`: if the timestamp advanced
and the cursor also moved, call it the mouse; if it advanced and the cursor sat
still, call it the keyboard.

That keeps the privacy and antivirus properties intact - a timestamp and a
cursor coordinate, still no key data and still no hook. It is a heuristic, and
worth being honest about where it is wrong:

- A mouse **click or wheel** with no movement reads as keyboard, and will wake.
  Acceptable: unlike a trackpad brush, clicking is deliberate.
- A trackpad touch too light to move the cursor reads as keyboard. In practice
  a brush that registers as input almost always moves the pointer.
- A keypress in the same 100 ms tick as a cursor move is attributed to the
  mouse and missed. Worst case is that waking waits for the next keypress.

Deliberately *not* solved with `GetAsyncKeyState` polled over the key range.
That would be exact, but sweeping every virtual key code in a loop is a
textbook keylogger signature - worse for Milestone 6 than the hook we already
rejected.

- **KEEPING_ALIVE** — WASAPI render stream open on the target device, emitting the configured keep-alive signal.
- **RELEASED** — stream fully closed (`IAudioClient` released, not merely paused — a paused stream may still hold the endpoint). Optionally, Bluetooth audio profile disconnected.

### 4.2 Idle detection

Poll the **device-level peak meter** (`IAudioMeterInformation` on the `IMMDevice`) at around 10 Hz.

This is simpler than enumerating individual audio sessions, and it works because our own keep-alive signal is vanishingly quiet — fluctuate mode peaks around 0.00003, far below any sensible threshold. So the device peak effectively reports *other* apps' audio, including JAWS speech.

- Peak above threshold → reset the idle countdown
- Countdown reaches zero → release

**Fallback if that proves unreliable:** enumerate sessions via `IAudioSessionManager2`, skip our own process ID, and check each session's state and peak. More code, more precise. Only go here if needed.

**RESOLVED ON HARDWARE (2026-08-30).** JAWS speech registers clearly, peaking around **0.56** - over half full scale, and roughly 1000x the 0.0005 threshold. Enormous margin. Device-level metering works, and the per-session fallback is not needed.

**Observability problem found during testing, worth designing around.** The tester could not cleanly observe the "audio stopped" transition, because reading the program's own output with JAWS generates speech, which trips the meter. Screen reader output and audio measurement interfere with each other by nature.

Consequence for Milestone 2: **the engine must log state transitions to a file, not just the console.** A log that can be read after the fact, when the headset is quiet, is the only way to verify idle behaviour without the act of observing changing the result. This will matter again when tuning the idle timeout.

**The signal/metering conflict is now moot.** It was real: sine at 1% (0.01) would have tripped our own detector and prevented release. But zeros won 2.3, and zeros has a literal peak of 0.0, so there is no self-detection risk at all on the default path. The coupling only returns if someone switches to sine for other hardware - at which point the app should either force the per-session method or refuse to combine sine with idle-based release. **Guard against that combination in code rather than leaving it as a trap.**

### 4.3 Settings

- Target device: default output, or a specific device (remembered by name, surviving re-plugging)
- Keep-alive signal: fluctuate / inaudible sine (frequency + amplitude) / pure zeros
- Release mode: idle-based (default) or fixed timer
- Idle timeout: seconds, default 30 s (settled 2026-08-30, confirmed 2026-09-05). **Must be easy to change from the dialog** - the tester asked for this directly.
- Fixed timer duration
- Global hotkey: user-assignable
- Wake on input: keyboard on by default; mouse a separate option, off by default (see 4.1)
- Start with Windows: on/off
- Earcons: on/off, volume

Dropped from this list: **hard release**, now deferred indefinitely (see 2.4).

Stored as a small config file next to the exe if that location is writable, otherwise in `%APPDATA%` — this keeps the portable build genuinely portable.

### 4.4 The control surface (Milestone 3)

**Hotkey.** `Ctrl+Win+F12`, chosen by the user. `RegisterHotKey` takes a
combination away from every other program for as long as we hold it, so the
default is deliberately obscure, and a combination with no modifier is refused
outright - registering a bare F12 would swallow that key system-wide. Registered
with `MOD_NOREPEAT`, so holding the keys toggles once. If registration fails the
app says so loudly: a hotkey that silently does nothing is the worst possible
failure for the primary interface.

**Earcons, and when the off-tone plays.** Rising two notes for on, falling for
off, with a few milliseconds of fade at each end - a sine that starts at full
amplitude clicks, and on headphones the click is more noticeable than the tone.

The non-obvious part is the ordering. The natural design, "release the device,
then play a tone", is wrong: opening the device again to play it would take the
headset straight back off the phone, a moment after handing it over. So the tone
is queued into the stream that is **already open**, and the stream closes once it
has drained.

That forced a second change. The render buffer was being filled as full as it
would go, which meant up to 500 ms of already-queued silence sitting in front of
anything new - so a tone would trail the keypress by half a second. `pump` now
maintains 200 ms of queued audio rather than filling the buffer, leaving 300 ms
of headroom for a descheduled thread. Measured: a release takes 125 ms without an
earcon and 340 ms with one, against a 1 s drain ceiling that exists so a device
which stops consuming cannot wedge the engine.

**Known gap:** an earcon can only play through an open stream, so switching off
when nothing is open is silent. Believed harmless - if no stream is open the
headset is not being held - but it is Test 18.

**Tray.** A convenience, never the only route to anything. Real Win32 menu, so
JAWS reads it unaided; state carried in the tooltip as words, with colour only as
a bonus for sighted users. The icon is drawn in code rather than embedded, which
costs a few hundred bytes of logic instead of a few kilobytes of resource. The
`TaskbarCreated` broadcast is handled, so the icon survives an Explorer restart.

**One hidden window, one message loop.** The window exists only to have a message
queue; the hotkey and the tray both post to it. Every message is handled in the
loop rather than in a window procedure, because a procedure would need global
state to reach the engine, and this way the whole control surface reads top to
bottom in one place.

### 4.5 Accessibility rules

These are requirements, not nice-to-haves:

1. Every function reachable by keyboard without the tray menu. Tray icons are awkward with a screen reader; the global hotkey is the primary interface, plus a second hotkey to open settings.
2. Real Win32 controls only.
3. Every control has a proper label and a sensible tab order.
4. The settings dialog is a real dialog — Escape cancels, Enter confirms, standard behaviour.
5. State changes are always audible (earcons), never visual-only.

---

## 5. Stack

- **Rust**, stable-msvc toolchain
- **`winsafe`** — native Win32 controls, settings dialog. Pinned to an exact version (it's 0.0.x and the API churns). **Not yet a dependency:** Milestone 3 needed a hidden window, a tray icon and a menu, none of which are *controls*, so it used the `windows` crate already present rather than pulling in a second GUI crate early. The choice for the Milestone 4 dialog is still open - `winsafe`, or raw `windows` with a real dialog resource. The constraint in CLAUDE.md is *real Win32 controls*, and both satisfy it; `winsafe` is a means to that end, not the end itself. Decide it at the start of Milestone 4, and weigh that `winsafe` wants to own the main window and message loop, which this app already has.
- **`windows-sys`** or **`windows`** — WASAPI (`IAudioClient`, `IAudioRenderClient`, `IMMDeviceEnumerator`, `IAudioMeterInformation`), plus `RegisterHotKey` and `Shell_NotifyIcon`
- **`embed-manifest`** — build dependency, for the app manifest
- Release profile tuned for size: `opt-level = "z"`, LTO, `panic = "abort"`, strip symbols

Target: single self-contained `.exe`, roughly 300 KB – 1 MB, no installer.

---

## 6. Open questions

### Resolved by Milestone 1 hardware testing (2026-08-30)

Raw results are in `test.txt`.

1. ~~Does soft release free the headset for the iPhone?~~ **Yes.** ~3 s handover, reproducible. See 2.4.
2. ~~Does JAWS speech show on the device peak meter?~~ **Yes,** peak ~0.56. See 4.2.
3. ~~Which keep-alive signal does the AeroClip need?~~ **Zeros is enough.** See 2.3.
4. ~~How long does the AeroClip take to reconnect after a hard release?~~ **Moot** - hard release is no longer being built.

### Resolved by Milestone 2 hardware testing (2026-09-05)

Raw results are in `test.txt`, Tests 8-10.

5. ~~What idle timeout feels right?~~ **30 s**, confirmed in use ("did the 30 second timeout feel better than 60? Yes"). The tester added that changing it should be easy - a requirement for the Milestone 4 dialog, not just a default.

8. ~~Does device-loss recovery actually work?~~ **Yes.** Disconnecting the AeroClip mid-session and reconnecting it ten seconds later brought keep-alive back with no typing, no stuck state and no repeated messages, and the log told the story clearly. The Test 5 failure is closed. One reporting wrinkle came out of it - see Milestone 2 in section 7.

9. ~~Does waking on input become annoying in practice?~~ **No**, and the disarm-on-manual-off rule did its job: after switching off by hand, typing did not take the headset back and the iPhone could claim it. But the tester asked for one change - **the mouse should not wake it**, because they do not use a mouse and can brush the trackpad by accident. Now design; see 4.1.

### Still open

6. **Does zeros still work after a much longer idle gap?** Testing used 20-30 second gaps. Real use involves far longer ones - an hour away from the desk, or a laptop that has slept. If clipping reappears after a long gap, fluctuate is the next thing to try. **Residual risk, watch for it in daily use.**

7. **Does the AeroClip behave the same on battery, or after a Windows sleep/resume cycle?** Sound Keeper carries explicit handling for modern-standby suspend/resume events, which suggests this bites in practice. Not yet tested.

### Opened by Milestone 3, for the next hardware round

10. **Do the earcons sound right?** Volume, length and shape are all guesses. 20 % amplitude, two notes, under 200 ms. Test 12.

11. **Do the automatic earcons become a nuisance?** This is the one to watch. Every time keep-alive re-arms on a keypress you now hear a rising tone, and every automatic release 30 s later gives you a falling one. Over a working hour that could be a handful of tones, or it could be maddening. If it is, the fix is small - an option to sound only on changes the user asked for - but it should not be built on spec. Test 17.

12. **Is the off-tone heard in full before the headset lets go?** The whole ordering design rests on it. Test 13.

13. **Does `Ctrl+Win+F12` clash with anything in JAWS?** Chosen because Windows and the Game Bar leave it alone, but the tester's JAWS setup is the thing that matters and has not been checked. Test 14.

---

## 7. Plan

**Milestone 0 — toolchain. DONE (2026-08-29).** rustup + Rust 1.98.0, Visual Studio Build Tools 17.14.39 with the VC++ workload. Verified end to end: a real binary compiles, links and runs.

**Milestone 1 - spike. DONE (2026-08-30).** All three questions answered; see section 6 and `test.txt`. The tester reported the spike itself was comfortable to use with JAWS, so the line-based, transition-only output pattern is worth reusing for future diagnostic tools.

**Milestone 1, original description.** `spike/` is a throwaway console tool that opens a WASAPI stream, emits a selectable keep-alive signal, and fully releases the device on command. Line-based commands and transition-only output, so it is usable with a screen reader.

Run with `cargo run` from `spike/`. Commands: `on`, `off`, `zeros`, `fluct`, `sine`, `watch`, `status`, `quit`.

Connect the AeroClip and make it the default output device first — the spike targets the default endpoint and reports which one it picked at startup.

**Milestone 2 - core engine. DONE (2026-08-30), hardware round closed 2026-09-05.**

Built: the three keep-alive signals, device selection by name with fallback to default, the state machine, device peak-meter idle detection, clean stream teardown, file-based transition logging, and a plain-text config file.

Verified automatically:
- Fixed-timer release fires at the right moment and logs it.
- Idle release fires after the timeout with no audio present.
- Device selection by name works; the AeroClip is visible as "Headphones (soundcore AeroClip)".
- 5 unit tests over config parsing and the validation guards.
- `cargo clippy -- -D warnings` clean.
- Release binary is **246 KB**, comfortably inside the ~1 MB budget.

**Hardware test results (2026-08-30), raw notes in `test.txt`:**
- Test 4, speech postpones release: **passed.**
- Test 5, following a device change: **failed.** Disconnecting the headphones stopped the engine for good; the user had to type `on` again. Fixed - see below.
- Test 6, log readability: **passed** ("Yes, clear", nothing missing).
- Test 7, general use: **passed**, no clipping. Timeout preference: 30 s.

**Fixes applied after testing:**

1. **Device loss no longer kills the engine.** The cause of Test 5 was conflating "should keep-alive be running" with "is a stream open". They are now separate: losing a device clears the stream but leaves intent standing, so the engine retries every 2 s and reopens on whatever Windows switched to. Retry failures are logged once, not repeatedly. Bounded naturally by the release timeout, so a headset that never returns does not retry forever.
2. **Wake on input**, at the tester's suggestion. See section 4.1.
3. **Default idle timeout 60 s to 30 s**, at the tester's preference.
4. **Fixed a bug in the wake feature found by its own test.** The check short-circuited on the config flag, so while waking was off the input watcher never refreshed its baseline; switching it on then compared against a stale timestamp and fired a spurious wake immediately. The watcher is now polled unconditionally and the flag consulted afterwards.

**Second hardware round (2026-09-05), raw notes in `test.txt`:**
- Test 8, device-loss recovery: **passed.** Disconnect, wait, reconnect - it came back on its own. The Test 5 fix holds.
- Test 9, waking on input: **passed**, including the part that matters most - after a manual `off`, typing did not take the headset back and the iPhone could claim it.
- Test 10, living with it: **passed.** No clipping, 30 s better than 60 s, waking never got in the way.

**One reporting wrinkle, worth knowing about.** During Test 8 the tester saw no `MOVED` or `INTERRUPTED` line, yet recovery plainly worked and the log recorded the device switch. That is the console harness, not the engine: `main.rs` drains the event channel only just before it blocks on `lines.next()`, so an event that arrives while it is waiting for input is not printed until the next time Enter is pressed. The engine writes the log file directly from its own thread, which is why the log was complete and the console was not.

Not worth fixing in the harness - Milestone 3 makes state changes audible through earcons, which is the real answer, and the log already covers after-the-fact reading. Recorded because it will otherwise look like a bug the next time somebody reads a transcript.

**Change requested and carried forward:** waking on input should ignore the mouse (now design, see 4.1), and the idle timeout needs an easy way to change it (Milestone 4).

The console harness in `main.rs` was scaffolding for testing the engine. Milestone 3 adds the tray and hotkey **alongside** it rather than replacing it, so there is still a diagnostic interface while the GUI is unproven; Milestone 6 removes it.

**Milestone 3 - control surface. BUILT (2026-09-05), pending a hardware round.**

Built: the global hotkey with its own parser and a refusal to register a bare key; earcons rendered into the keep-alive stream, with an anti-click envelope; the tray icon, its real Win32 menu and an icon drawn in code; the hidden window and message loop; and the keyboard-only wake change carried over from Milestone 2's round. Design detail in 4.4.

Verified automatically:
- Startup registers `Ctrl+Win+F12` on this machine, so nothing here is holding it.
- A full on/off cycle opens and releases the AeroClip cleanly, with no hang and no retry storm.
- Release latency measured at 125 ms with earcons off and 340 ms with them on, well inside the 1 s drain ceiling - so `drain` is genuinely draining rather than timing out.
- Config round-trips every new setting; a mistyped hotkey is reported rather than silently dropped.
- 24 unit tests. `cargo clippy --all-targets -- -D warnings` clean.
- Release binary **266 KB**, up from 248 KB, against the ~1 MB budget.

**Not tested on hardware. None of it.** Whether the tones are audible and pleasant, whether the off-tone is heard in full before the headset lets go, whether the hotkey clashes with JAWS, whether the tray menu reads properly, and whether automatic earcons become a nuisance. `test.txt`, Tests 11-18.

Two design points settled while building:

- **Earcons play on the existing keep-alive stream, before it is torn down**, and the queued-audio target dropped from 500 ms to 200 ms so the tone does not trail the keypress. See 4.4.
- **The console harness stays**, on its own stdin thread feeding the same command channel as the hotkey and tray.

**Milestone 4 — settings.** The `winsafe` dialog, config load/save, second hotkey to open settings. Test the whole dialog under JAWS with the screen off.

**Milestone 5 - hard release. DEFERRED, probably not needed.** Soft release was confirmed sufficient on the AeroClip (see 2.4), so this is no longer planned work. Revisit only if another headset needs it, or if the 3-second handover becomes annoying. Reference if it ever happens: `m2jean/ToothTray`.

**Milestone 6 — ship.** Autostart, size-tuned release build, README, and a plan for the antivirus/SmartScreen problem — a small unsigned binary that opens audio devices and registers global hotkeys fits the profile AV heuristics dislike. Options: submit false-positive reports to the major vendors, or look at code signing.

**Current position (2026-09-05):** Milestones 0, 1 and 2 complete and hardware-tested. Milestone 3 built and awaiting its hardware round.

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
