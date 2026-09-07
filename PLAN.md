# StableSound - research findings and build plan

## 0. Where we are (updated 2026-09-08)

**Read this first when picking the project up.**

Milestones 0 to 5 are complete, hardware rounds included, and so is the small
follow-up round 5.1. Milestone 3 took **three** rounds. Milestones 4, 5 and
5.1 took one each and all three passed.

**Milestone 5.1 passed cleanly - every test, first time.** The tray icon reads
its state once however many times it changes, a hotkey works on a punctuation
key, a refused setting is read back with the arrow keys, and the two reversed
defaults are what a new user meets. The tester's verdict on the whole app:
"all works really well and I'm very pleased", and on the plan for Milestone 6:
"this will be a great first version." Raw results in
`test-milestone-5-1.txt`, Tests 39-43.

**One thing came back from it**, and it is built: the heading over the message
window now says what kind of message it is rather than how to read it. See
4.7.

### What exists and works

A finished app, waiting on its last hardware round. It keeps the headphones
awake, releases them on a timer, wakes again when you touch the machine, has a
tray icon, a global hotkey, a real settings dialog and a help document, and can
log to `stablesound.log` next to the exe.

**No console window since Milestone 6**, and no window of its own at all until
it is asked for one. Where the console's output went is described in 6.1.

Release binary is **294 KB**, against a ~1 MB budget.

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
- `Ctrl+Win+F12` works from every program tried - a word processor, Chrome and
  the desktop - toggles exactly once when held down, and clashes with nothing in
  the tester's JAWS setup.
- The off-tone is heard **in full**, every time, before the headset is let go,
  and the iPhone then claims it immediately. The ordering design in 4.4 is
  sound; this was the question the whole of Milestone 3 rested on.
- With no headphones connected at all, the hotkey is harmless - the tones simply
  play through the laptop speakers.
- The earcons are right: volume, length, shape, no clicks, and the whole
  on-tone audible from its start. Signed off in the second round.
- Automatic transitions are silent and stay silent. "I heard no tones, which
  was perfect."
- The tray works from the keyboard. One icon, read once and correctly, `Enter`
  toggles once per press, the menu opens on the Applications key.
- **Waking still works with the mouse handling gone**, and the trade it cost
  is accepted: "No, its fine. I've learned to avoid touching it." A manual
  switch-off still keeps the headset free whatever is touched afterwards,
  which was the part that mattered most.
- **The version 6 common controls changed nothing about how the dialog reads.**
  "Everything still worked the same. No issues while tabbing through."
- **The reworded labels all read on their own.** "All items read clearly and
  are easy to understand."
- **The settings dialog reads correctly under JAWS.** Every control announced
  with its name and its value, mnemonics working, the choices in the combo
  boxes sensible, a refused hotkey read aloud with the focus returned to the
  field that was wrong, and the toggle hotkey still working while the dialog is
  open. The tester's verdict: "This was perfect. The dialogue behaves like a
  model accessible app." Signed off in the Milestone 4 round.

### What the hardware rounds asked for

Every request from Milestones 2 and 3 is now built, and all but one is
confirmed. The exception is the one that was abandoned instead:

- **Do not wake on the mouse. ABANDONED after four rounds**, at the tester's
  own request: "Please give up on this feature now and remove all code that was
  there to facilitate it. It was just a nice-to-have anyway and I'm happy to let
  it go for now." Three mechanisms were built and every one of them failed on
  the trackpad. See 4.1 for what was tried and why it kept losing.
- **Make the timeout easy to change.** Done in Milestone 4 and confirmed.
- **Tones only for what the user does.** Done, and confirmed.
- **Tone volume, length and the clipped on-tone.** Done, and confirmed.
- **The tray.** Fixed and signed off in the third Milestone 3 round.
- **A settings item in the tray menu.** Done in Milestone 4 and confirmed.

The Milestone 4 round then asked for five more, all built in Milestone 5:

- **Drop the mouse feature entirely**, above.
- **Announce the group headings.** JAWS named every control but never the group
  it was in. **This one could not be delivered** - see question 21. What was
  done instead is that no control now depends on its group heading to make
  sense, which was the underlying complaint: "Remember that a screenreader user
  hears options in isolation."
- **Make the settings hotkey optional.** "It won't be used often enough to
  warrant a hotkey, so having it in the tray only is my current preference.
  Maybe make the option to have a settings hotkey available in the settings
  dialogue, offering the current ctrl+win+f11 option by default." Done exactly
  that way, and off by default.
- **Say how to write a hotkey when one is refused**, with every modifier
  spelling, and whether the order matters. Done; it does not.
- **A steer on logging.** Asked for directly, given in 4.6, and then
  overruled by the next round. See 4.7.

The Milestone 5 round then asked for five more, all built and described in
4.7: the tray icon reading its state twice, logging off by default, the
settings hotkey on by default after all, punctuation keys in hotkeys, and
messages put somewhere the arrow keys can review them.

And two things were confirmed for later rather than acted on now:

- **A help file, with a button in the dialog and the tray menu.** Wanted
  "eventually". Milestone 6, once there is something to open.
- **The console window can go.** "The dialogue is enough and I would be
  comfortable with the console window removed." Milestone 6, as planned.

### Known residual risks, still untested

- Whether zeros still works after a much longer idle gap - an hour away from
  the desk rather than 30 seconds (question 6). Detailed logging now records
  what the meter saw, so a recurrence leaves evidence.
- Behaviour across Windows sleep/resume, and on battery (question 7).
- Whether the reworded message heading reads well under JAWS (question 26).
  Checked the same way its predecessor was - by reading what UI Automation
  hands a screen reader - but not heard spoken. It rides along with the
  Milestone 6 round.

### What is next

**The Milestone 6 hardware round, and then shipping.** Milestone 6 is built:
the console is gone and the binary is a windows subsystem one, there is a help
document in HTML with buttons in both the dialog and the tray menu, a second
copy of the app is prevented and says so, the README and LICENSE are written,
and the antivirus question has been researched and answered (6.5).

The script is `test-milestone-6.txt`, Tests 44-51. It carries question 26 - the
reworded message heading, built after the 5.1 round and still unheard - along
with everything Milestone 6 added.

**Nothing else is outstanding.** After that round it ships.

### Things a fresh session should not re-litigate

- Real Win32 controls from a **dialog template compiled by rc.exe**. Not a
  drawn-UI toolkit, and not `winsafe` either - see section 5. JAWS is the whole
  point. `native-windows-gui` is unmaintained.
- Hard release (disconnecting Bluetooth) is **not** being built. Soft release
  was proven sufficient.
- Keep-alive never starts because audio was detected. Only explicit request or
  user input. Section 4.1 explains why.
- Zeros is the default signal. Sine is a last resort and forces fixed release.
- **The group headings are closed.** JAWS does not announce them, "Show
  control group info" was already on, and the tester has ruled: "due to the
  options being very clear, this does not matter... no further work on this is
  needed."
- **Telling the keyboard from the mouse is closed.** Four rounds, three
  mechanisms, the tester's own decision to drop it. Do not reopen it without
  being asked.
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
| Wake on input | **Any input, on by default** (revised 2026-09-05) | Requested after Milestone 2 testing. Once idle release has fired, the next sound is often a notification, and its first word gets clipped. Touching the machine is a reliable sign speech is about to be wanted. Switching off by hand disarms it, so releasing for the phone still sticks. The same round asked for the *mouse* to be excluded; three mechanisms were built for that over four rounds, all of them failed on a real trackpad, and the tester asked for the feature to be dropped. See 4.1. |
| Idle timeout | **30 s** (revised 2026-08-30) | 60 s felt too long in use once waking on input made re-arming cheap. |
| Feedback | **Earcons** (distinct short tones for on/off) | Played through the target device, so hearing the tone also proves the headset is awake. Optional speech can layer on later. |

---

## 4. Design

### 4.1 States

```
RELEASED       --hotkey--------------------------->  KEEPING_ALIVE
RELEASED       --user input, if armed------------->  KEEPING_ALIVE
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
testing, at the tester's suggestion). Typing and pointing are not caused by our
own output, so they cannot form the feedback loop that makes audio-waking
useless. It also closes a real gap: after an automatic release, the next thing
to make a sound is often a notification, and that first word would be clipped.

Input is detected with `GetLastInputInfo`, deliberately not a `WH_KEYBOARD_LL`
hook. A small unsigned binary that reads every keystroke is exactly the shape
antivirus heuristics flag, which this project already expects to fight at
Milestone 6. `GetLastInputInfo` returns only a timestamp, never key data.

**Keyboard versus mouse: tried three ways, and dropped (closed 2026-09-05).**
Milestone 2's round asked for the mouse to stop waking keep-alive: the tester
does not use one, and brushing the trackpad by accident takes the headset back
off the phone. `GetLastInputInfo` reports a single timestamp for all input and
cannot say what caused it, so something had to supply the missing half. Three
mechanisms were built, and **every one of them failed on the trackpad**:

1. Pair the timestamp with `GetCursorPos`: pointer moved, call it the mouse;
   pointer still, call it the keyboard. Defeated by the *end* of a sweep -
   lifting a finger is itself an input event, arriving after the pointer has
   already stopped, which is the exact signature of a keypress.
2. The same, with a settling window requiring the pointer to have been still
   for 500 ms. A sweep still woke keep-alive every time, short flicks included.
3. Raw mouse input, so the mouse reported itself rather than being inferred -
   usage page 1, usage 2, `RIDEV_INPUTSINK`, payload never read. This mechanism
   was **correct**: the diagnostic log showed 42 events attributed properly.
   It still failed, because of a race between two clocks rather than anything
   about identifying the device. `GetLastInputInfo` is stamped the instant
   input lands; `WM_INPUT` must be queued and dispatched to us afterwards, so
   a poll landing in the gap finds a stale pointer tick. Only the *leading*
   event of a sweep can lose that race - and the leading event is the one that
   wakes, which is why the fault looked like a misidentification for two
   rounds. Holding an unaccounted-for input for 250 ms closed that race, was
   measured from the tester's own log rather than guessed, was unit-tested
   including the failing case, and the trackpad woke keep-alive anyway.

After the fourth round the tester called it: "Please give up on this feature
now and remove all code that was there to facilitate it. It was just a
nice-to-have anyway and I'm happy to let it go for now."

**So all input wakes keep-alive, and nothing tries to say what caused it.**
`wake_on_mouse` is gone, along with the raw input registration and its
system-wide `WM_INPUT` stream, the settling window and the held decision.

What that costs is bounded and known: a trackpad brush takes the headset back
from the phone, and the hotkey takes it straight off again. The property that
makes a deliberate handover stick is untouched - switching off *by hand* still
disarms waking entirely, so a stray finger cannot undo it.

What it buys back: waking is immediate again rather than a quarter of a second
late, there is no `WM_INPUT` for every pointer movement on the machine, and the
app is materially smaller and simpler.

There is a lesson here worth keeping, because it cost four rounds. The
mechanism was right by round three and the diagnosis was right by round four,
and it still did not work. **A feature nobody would miss should be given a
budget before it is started, not after.**

Deliberately *not* solved with `GetAsyncKeyState` polled over the key range.
That would be exact, but sweeping every virtual key code in a loop is a
textbook keylogger signature - worse for Milestone 6 than the hook already
rejected above.

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

**What the meter is asked to say now (added 2026-09-05).** Detailed logging
records the two threshold crossings and the level each was decided on -
`audio detected, peak 0.4935` and `audio stopped, peak 0.0000; letting go in
4s unless something plays`. The transitions only, not the ten readings a
second in between. This exists because of the observability problem two
paragraphs up: the two questions still open about the meter, whether digital
silence still works after a long gap and what a sleep or resume does, cannot be
watched happening and can only be read back afterwards. See 4.6.

**Telling the keyboard from the mouse: closed, unsolved (2026-09-05).** Three
mechanisms over four hardware rounds, the last of them correct about the
mechanism *and* correct about the race that broke it, and the trackpad woke
keep-alive every time. Dropped at the tester's request; the full account is in
4.1, because what remains is a decision rather than a design.

### 4.3 Settings

All of these are now in the dialog (Milestone 4) unless marked otherwise:

- Target device: default output, or a specific device (remembered by name, surviving re-plugging)
- Keep-alive signal: fluctuate / inaudible sine / pure zeros. **The sine frequency and amplitude are not in the dialog** - they are a last resort for hardware the other two fail on, they mean nothing without each other, and two more numeric fields in front of every user is a poor trade for that. The config file still carries them, and choosing the tone in the dialog keeps whatever is in the file.
- Release mode: idle-based (default) or fixed timer
- Idle timeout: seconds, default 30 s (settled 2026-08-30, confirmed 2026-09-05). **Must be easy to change from the dialog** - the tester asked for this directly.
- Fixed timer duration - the same field; only one mode applies at a time
- Global hotkey: user-assignable. **Typed, not captured** - see below.
- Second global hotkey to open the dialog, **off by default**, with `Ctrl+Win+F11` offered and kept in the file so switching it on is one checkbox (see 4.6)
- Wake on input: on by default. Any input, with no attempt to tell the keyboard from the mouse (see 4.1)
- Start with Windows: on/off. **Kept in the registry, not the config file** - see below.
- Earcons: on/off, volume (shown as a percentage; "10" is far easier to hear and retype than "0.1")
- Logging on/off, and detailed logging for troubleshooting, both off by default (see 4.7)

Dropped from this list: **hard release**, now deferred indefinitely (see 2.4).

Stored as a small config file next to the exe if that location is writable, otherwise in `%APPDATA%` — this keeps the portable build genuinely portable.

**The hotkeys are typed, not captured.** Windows has a `HOTKEY` control that records whatever combination you press into it. It is wrong here twice over: a screen reader user pressing a combination gets no readable confirmation of what was captured, and the control cannot express the Windows key at all - which the default `Ctrl+Win+F12` needs. A plain edit box holding `ctrl+win+f12` is readable, reviewable, correctable a character at a time, and parsed by the same `Hotkey::parse` that reads the config file, so the dialog and the file cannot disagree.

**"Start with Windows" is not a config setting.** What decides it is a value under the `HKCU` `Run` key. A copy in `stablesound.conf` could disagree with it - somebody clears the value with an autoruns tool, or copies a config between machines - and the checkbox would then report something untrue. The registry is the single source of truth and the dialog reads it directly.

**Two identical hotkeys are pulled apart by `validate`.** A combination can be registered once; the second registration fails and that function is left with no key at all. The dialog refuses it up front and the config loader corrects it.

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

**Only for what the user does (revised 2026-09-05).** Tones originally marked
every state change, including automatic ones. The hardware round was blunt about
it: heard "too often", "annoying", and the tester asked for the automatic
release and the wake that follows it to be "seamless, either silent or maybe a
small pop, but silence will likely be my preference". They are now silent,
unconditionally, whatever `earcons` is set to - it is not a third setting.

The reasoning holds up independently of taste. An automatic release and the wake
after it happen many times an hour and change nothing the user can act on: only
which device currently holds the headset. A hotkey press is the opposite - it
has no other feedback at all, so it must be answered. Implemented as a one-shot
`announce` flag set only by an explicit request. It survives a *failed* open, so
a hotkey pressed while the headset is still reconnecting is answered late rather
than not at all, and it does not survive the open it belongs to, so a later
reconnect or a move to another device stays quiet.

**Lead-in silence.** The round also found the start of the on-tone missing.
Writing samples the instant after `IAudioClient::Start` is not the same as those
samples reaching the ears: a Bluetooth headset needs a moment to bring its link
up, and what is written during it is lost. There is no event that says "you are
audible now", so the on-tone simply begins with silence. Only the on-tone -
putting a delay in front of a *release* is the one thing this design refuses to
do.

250 ms fixed the clipping and the second round confirmed it, but the tone then
felt slow to arrive, so it is now 125 ms. The tester's reasoning was better than
a preference: "I can hear JAWS starting to announce the keystroke prior to the
tone playing, so it should be safe to halve the delay." JAWS beginning to speak
is itself evidence the link was already up.

The non-obvious part is the ordering. The natural design, "release the device,
then play a tone", is wrong: opening the device again to play it would take the
headset straight back off the phone, a moment after handing it over. So the tone
is queued into the stream that is **already open**, and the stream closes once it
has drained.

That forced a second change. The render buffer was being filled as full as it
would go, which meant up to 500 ms of already-queued silence sitting in front of
anything new - so a tone would trail the keypress by half a second. `pump` now
maintains 150 ms of queued audio rather than filling the buffer, leaving 350 ms
of headroom for a descheduled thread, against a 1 s drain ceiling that exists so
a device which stops consuming cannot wedge the engine.

The tester found the resulting release "noticeable after you pointed it out. Not
a big deal", and asked for slightly snappier. Two things now give that: the notes
are a fifth shorter, and the queue target came down from 200 ms to 150 ms.
Measured on this machine, a manual release settled at 260-350 ms with earcons and
120-190 ms without. The first cycle after the headset has been sitting idle is
slower - 700 ms, and once 1.6 s - because a cold Bluetooth stream drains lazily;
that is the same effect the lead-in silence exists for. An **automatic** release
now costs nothing at all, since it no longer plays or drains anything.

**Known gap, confirmed harmless.** An earcon can only play through an open
stream, so switching off when nothing is open is silent. Test 18 found this a
non-issue in practice: pressing the hotkey twice quickly was "clear from the
tones", and with the headphones disconnected entirely the tones just came out of
the laptop speakers "with no ill effects".

**Tray.** A convenience, never the only route to anything. Real Win32 menu, so
JAWS reads it unaided; state carried in the tooltip as words, with colour only as
a bonus for sighted users. The icon is drawn in code rather than embedded, which
costs a few hundred bytes of logic instead of a few kilobytes of resource. The
`TaskbarCreated` broadcast is handled, so the icon survives an Explorer restart.

**Two things were wrong, and the first fix only found one of them.** The tray
was unusable from the keyboard - no menu from the Applications key or
`Shift+F10`, and `Enter` on the icon did nothing - and it stayed that way
through a second round.

**`NIM_SETVERSION` is not optional.** The icon had never asked for version 4 of
the shell's notification protocol. Without that request the shell speaks the
original Windows 95 protocol, in which a tray icon hears about mouse buttons and
nothing else, so keyboard activation produces no message whatsoever.

**But sent messages never reach a message loop, and that was the real fault.**
`GetMessage` returns *posted* messages. Messages that are *sent* it dispatches
straight to the window procedure while it waits, and never returns to the
caller. The hotkey and the timer are posted, so they arrived and the loop looked
healthy. The shell sends the tray callback - so it went to a window procedure
whose entire body was `DefWindowProcW`, and was discarded. Every click, every
`Enter`, every Applications key, silently thrown away, in both versions of the
protocol. Reading the tray off the loop could never have worked.

`wndproc` now re-posts the callback as `WM_TRAY_QUEUED`, which puts the decision
back in the loop where the rest of the control surface lives, and works whether
the shell sends the message or posts it. It is the right order of events anyway:
showing a modal menu inside a sent message blocks the sender.

**Demonstrated, not assumed.** A test sends `WM_TRAY` to the running app's
window from another process, exactly as the shell does, and confirms that
`NIN_SELECT` and `NIN_KEYSELECT` toggle keep-alive and that a doubled `Enter`
toggles once. Version 4 is also confirmed accepted on this machine, which is how
we know it was never the whole story.

Version 4 adds `NIN_SELECT`, `NIN_KEYSELECT` and `WM_CONTEXTMENU`, and it puts
the icon's own screen position in `wParam`, so a keyboard-opened menu now appears
beside the icon rather than beside the mouse pointer. It also needs `NIF_SHOWTIP`
to keep the ordinary tooltip, and it reverses the `wParam`/`lParam` layout of the
callback. The version 3 mouse messages are accepted too, and a refused
`NIM_SETVERSION` is no longer fatal: handling both costs a few lines and removes
the question of which protocol is in force from the next round entirely.

One guard came with it: the shell has a long-standing habit of sending
`NIN_KEYSELECT` twice for a single `Enter`, and accepting both protocols gives a
second way for one press to arrive twice. Two toggles in a row cancel out -
which looks exactly like an icon that does nothing, the very symptom being fixed
- so a second activation within 250 ms is treated as an echo.

**Ghost icons, and the doubled name.** Both rounds reported the icon's name read
twice over, once with the old wording and once with the new. The most likely
explanation is that there were two icons: closing the console window or pressing
Ctrl+C does not unwind and does not run `Drop`, so the icon stays in the
notification area pointing at a dead window. A ghost is worse than untidy here,
because it is indexed and read out exactly like the live one - and pressing
`Enter` on a ghost does nothing, which is indistinguishable from the bug above.
`SetConsoleCtrlHandler` now removes the icon on the way out of a killed process.

That prevents new ghosts; it cannot clear ones already there, which a reboot
does. Whether it was ever the explanation is still open - see question 15 - and
every tray callback is now logged so the next round can settle it: if `Enter`
produces no log line, the icon being pressed is not ours.

`NIF_GUID` would give the icon an identity stable across runs and let a ghost be
deleted outright at startup, and was rejected: it binds the icon to the
executable's path, and a single portable exe the user is expected to move is
exactly the case it breaks.

The tooltip was also cut down. It is the icon's accessible name, read out in full
every time the user arrows onto it, and the first attempt was judged "too
verbose": "StableSound: released, the headphones are free" is now "StableSound:
headphones free".

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

### 4.6 What the Milestone 4 round settled (Milestone 5)

**The settings hotkey is optional, and off.** *(Reversed by the Milestone 5
round: it is optional, and on. See 4.7. The reasoning below is unchanged and
still explains the checkbox.)* `Ctrl+Win+F11` opened the dialog
from anywhere. The tester's answer to whether that was worth a combination
taken from every other program: "It won't be used often enough to warrant a
hotkey, so having it in the tray only is my current preference. Maybe make the
option to have a settings hotkey available in the settings dialogue, offering
the current ctrl+win+f11 option by default."

So a checkbox decides whether it is claimed, the field beside it stays filled
with `Ctrl+Win+F11` whether or not the checkbox is ticked, and the default is
off. Turning it off actually drops the registration, which is the whole point;
turning it on claims it without a restart.

This brushes against CLAUDE.md's rule that the tray must never be the only
route to a feature, so it is worth being explicit that it is not. With the
hotkey off, the dialog is reachable from the tray - which is keyboard-driven
with `Win+B` and was signed off in the third Milestone 3 round - and from the
console's `settings` command until Milestone 6 removes it. The hotkey remains
one checkbox away. The rule exists because tray menus are awkward with a screen
reader, not because they are forbidden, and the tester chose this.

**A refused hotkey now says how to write one.** The message read correctly and
returned the focus to the right field, and the tester asked it to go further:
"give examples of how to correctly write all the possible modifier keys... If
the order of modifier keys is important, make that clear."

`Hotkey::parse` reports which of five mistakes was made - nothing typed, an
unknown word, two keys, modifiers with no key, a key with no modifier - so the
message names the actual fault. After it comes one shared `HOW_TO_WRITE`
string, used by the dialog, the console and the config loader alike, listing
every accepted modifier spelling and every key name. A test asserts that
everything the parser accepts appears in it, so the two cannot drift apart -
which is the failure mode the tester actually met.

**The order does not matter, and the text says so**, along with capitalisation
and spacing. A test pins that too.

**Every control reads on its own.** "Remember that a screenreader user hears
options in isolation. e.g. for me, as the developper, its obvious that the 'let
go of them' option refers to the headphones. This may however not be obvious to
a new user." No label in the dialog now depends on another, or on its group
heading, to make sense.

**Logging: a split, not a choice.** Asked for directly - "I don't foresee
myself or other users wanting to keep a log, but understand that this is
helpful for development and trouble-shooting. So maybe make a 'debug mode'
checkbox available... But I'd actually like your steer on best practise here."

The steer taken, and the reasoning, because this is a judgement rather than a
finding:

- **The ordinary log stays on.** It is one line per state change - a few
  hundred bytes a day - and it is the only evidence that exists in an app whose
  behaviour cannot be watched as it happens. Every hardware round so far that
  produced a fix rather than a guess produced it from this file, and the ones
  that produced guesses were the rounds before it existed. A log that is off
  when the problem happens is worth nothing.

  **Overruled by the Milestone 5 round, and the log now ships off.** The
  argument above was put and the tester chose otherwise having seen both. See
  4.7 for what that costs and who has to carry it.
- **Detailed logging stays off** until something is being looked into, because
  it is several times longer and only useful to somebody reading it closely.
- **What "detailed" now means** is the peak meter crossing the audio threshold
  in each direction, with the level, and how long each device took to open.
  Those are exactly the measurements behind questions 6 and 7. It previously
  meant the keyboard-or-mouse reasoning, which no longer exists.

**The app finally has a manifest.** Found while checking the dialog from
outside: there was none at all, though section 5 had assumed one, so the
process got the version 5 common controls. That made this the only dialog on
the machine not built from the same controls as every other - which is the
entire argument for using a template rather than a toolkit. It is embedded
through the `.rc` that rc.exe already compiles, so it costs no dependency
against the size budget.

Being straight about it: this was tried as a fix for the unannounced group
headings and made no difference at all to the accessibility tree. It is kept
because an app with a dialog should have one, not because it solved anything.

### 4.7 What the Milestone 5 round settled (Milestone 5.1)

Five things, none of them large, and one of them a shell bug rather than ours.
**All five passed their round on 2026-09-07**, first time and without
qualification. What each one was, and what the round said back:

**The tray icon read its own state twice.** "With keep-alive off, it correctly
displays StableSound: headphones free. With keep-alive on, it displays
StableSound: headphones free StableSound: headphones awake."

Windows 11 keeps whatever `szTip` said at `NIM_ADD` as the icon's *label*, and
from then on hands a screen reader `"<label> <tooltip>"`, printing it once only
while the two agree. Registering the icon under its off-state text therefore
guaranteed the doubling the moment the state changed. This is the shell's rule,
not a mistake of ours: on the same machine Microsoft's own Windows Security icon
reads "Windows Security - No actions needed. Windows Security - Actions
recommended."

So the icon is added as `StableSound` and the tooltip carries the state alone:
"StableSound Headphones awake". That is the shape every built-in icon already
uses - "Volume Headphones (soundcore AeroClip): 44%" - and it is shorter than
what it replaced. Confirmed against the live notification area, off and on and
off again, by reading the name UI Automation exposes.

Confirmed by ear in the round, including the part that mattered - that it
holds across repeated changes of state, which is where the old fault only ever
showed: "Yes, it stays one phrase, behaving perfectly."

**Logging is off by default.** Asked for directly, against the steer given in
4.6: "Please have logging off by default and it can then be turned on when
needed for development or trouble-shooting." The argument for keeping it on is
unchanged and was put; the tester heard it and chose otherwise, having seen
both. **The consequence has to be carried by the test scripts from now on: a
round run without switching logging on first leaves nothing behind to read.**
Every script from Test 39 onwards says so in its opening steps.

Both defaults were met cold in the round, as a new user would meet them, and
both read right. On whether the checkbox label now carries the weight the
default used to - "is it clear enough that you would know to tick it before
trying to reproduce a problem?" - the answer was yes.

**The settings hotkey ships on.** The reverse of what Milestone 4 asked for,
decided after living without it: "Actually, ship with the settings hotkey on.
In the documentation later, we can recommend that the hotkey can be disabled
once the user has completed tweaking the application settings to their
satisfaction." The reasoning in 4.6 survives intact, only inverted - the moment
to hand `Ctrl+Win+F11` back to the rest of the machine is once the settings are
the way you want them, not before you have ever opened them. The help file owes
this a paragraph.

**Hotkeys can use punctuation keys.** "I tried using ctrl+win+; but ; was not
available as an option. If there isn't a good reason for not having punctuation
keys available, please fix this." There was no good reason, only a table that
stopped at letters, digits and function keys.

Punctuation has no fixed virtual key code the way those do - the key that types
`;` sits somewhere else on another layout - so the mapping comes from
`VkKeyScanW` and is read back for display by `MapVirtualKeyW`. Any single
character is now accepted. Where a character needs Shift, Shift is folded into
the combination, because that is what the fingers do: `ctrl+win+:` is
`Ctrl+Win+Shift+;` and is shown that way. AltGr characters are refused, since
they want Ctrl and Alt held down too and would fight the modifiers rather than
join them. The plus key is written as the word `plus`, a plus sign being what
joins the parts together.

Confirmed, including the Shift folding, which was the part most likely to
confuse: "that is exactly what it says. Happy with that." The tester settled
back on `Ctrl+Win+F12` in the end, because `Ctrl+Win+;` turned out to clash
with a Leasey keystroke - worth knowing when the README comes to recommend a
default, and an argument for the field accepting anything rather than a list.

**Messages are now read back with the arrow keys.** On the refusal advice: "Its
long, but instructive. A Jaws user needs to use the Jaws cursor or to
virtualise the control to absorb all the detail, which may not be within the
abilities of a more novice user. Putting the information in a read-only edit
field would solve this, by making it possible to use the cursor keys to review
the text. The text itself is excellent."

So the text stays and the window changes. `IDD_MESSAGE` is a second dialog
template whose first tab stop is a read-only multi-line `EDIT`: it is read on
focus the way a message box is, and then the arrow keys walk it a line or a
word at a time with no cursor mode to switch into. Every message the dialog
shows goes through it, not only the hotkey one. `MessageBeep` keeps the one
thing a message box gave that a plain dialog does not - the system ding that
arrives ahead of any speech and says which kind of message this is.

It is modal, as the message box was, so the global hotkey is not serviced while
it is up. That is unchanged rather than a regression, and it is a window that
exists to be dismissed.

The round confirmed the whole of it - reviewable a line and a word at a time,
"no cursor mode to switch into" borne out ("It just worked well from where the
focus was"), and question 25 answered against keeping a message box for the
short messages: "No, this is good as it is."

**And it asked for the heading to change.** The heading used to describe the
control - "Message, which the arrow keys will read back" - and JAWS said "read
only edit" straight after it, so two announcements were spent on the same
fact. "This already tells the user how to interact with the dialogue, so the
message box label could rather be something like: 'error, and how to fix'."

Which is right, and it also fills a real gap: which *kind* of message this was
had been carried only by the ding. The heading is now set at runtime and says
so in words - "What went wrong, and how to fix it:" for a refusal, "What
happened:" for a note. Both start with "What", so the `W` mnemonic holds
whichever is showing; that mnemonic is the way back to the text once Tab has
moved on to OK.

The static carrying it needed a control ID to be written to, where it had been
`-1`. That does not disturb the labelling: a dialog's label association is the
preceding static in tab order, not a particular ID, and the running window was
checked to be sure - UI Automation gives the focused edit the name "What went
wrong, and how to fix it:", with 817 characters of advice in it over 20 lines,
multi-line and read-only, and the focus back on the offending field once it is
dismissed.

**On the group headings, nothing changed and nothing more will.** The tester
checked the JAWS setting: "Its called 'show control group info' and it was
already on, which is the default setting." They still are not announced, and
the verdict is to leave it: "due to the options being very clear, this does not
matter... no further work on this is needed." Question 21 is closed unsolved,
with the mitigation - no label leaning on its heading - already in place and
confirmed to work.

---

## 5. Stack

- **Rust**, stable-msvc toolchain
- **`winsafe`** — **not used, and now decided against** (2026-09-05). The Milestone 4 dialog is built from a real Win32 dialog template compiled by rc.exe, driven with the `windows` crate the app already depends on. The constraint in CLAUDE.md is *real Win32 controls*, and `winsafe` was only ever a means to that end. Three things settled it: the template gives tab order, mnemonics, the default button, Escape and label-to-control naming from Windows itself, which is more than the constraint asks for; `winsafe` wants to own the main window and message loop, and this app already has one for the tray, hotkey and raw mouse stream; and it would cost a dependency against a hard size budget for controls that come free either way. The whole dialog added **22 KB**.

- **`embed-resource`** — a *build* dependency, running rc.exe over `stablesound.rc`. Nothing of it ships. Weighed against the size budget and it costs nothing at runtime; the compiled template itself is a couple of kilobytes.
- **`windows-sys`** or **`windows`** — WASAPI (`IAudioClient`, `IAudioRenderClient`, `IMMDeviceEnumerator`, `IAudioMeterInformation`), plus `RegisterHotKey` and `Shell_NotifyIcon`
- **The application manifest** - `stablesound.manifest`, referenced from `stablesound.rc` as `RT_MANIFEST` so rc.exe embeds it. **No crate**: `embed-manifest` was in this list for a long time and was never actually added, so until 2026-09-05 the exe had no manifest at all and got the version 5 common controls. Going through the `.rc` costs nothing, since it is compiled anyway. See 4.6.
- Release profile tuned for size: `opt-level = "z"`, LTO, `panic = "abort"`, strip symbols

Target: single self-contained `.exe`, roughly 300 KB – 1 MB, no installer.

---

## 6. Open questions

### Resolved by Milestone 1 hardware testing (2026-08-30)

Raw results are in `test-milestones-1-2.txt`.

1. ~~Does soft release free the headset for the iPhone?~~ **Yes.** ~3 s handover, reproducible. See 2.4.
2. ~~Does JAWS speech show on the device peak meter?~~ **Yes,** peak ~0.56. See 4.2.
3. ~~Which keep-alive signal does the AeroClip need?~~ **Zeros is enough.** See 2.3.
4. ~~How long does the AeroClip take to reconnect after a hard release?~~ **Moot** - hard release is no longer being built.

### Resolved by Milestone 2 hardware testing (2026-09-05)

Raw results are in `test-milestones-1-2.txt`, Tests 8-10.

5. ~~What idle timeout feels right?~~ **30 s**, confirmed in use ("did the 30 second timeout feel better than 60? Yes"). The tester added that changing it should be easy - a requirement for the Milestone 4 dialog, not just a default.

8. ~~Does device-loss recovery actually work?~~ **Yes.** Disconnecting the AeroClip mid-session and reconnecting it ten seconds later brought keep-alive back with no typing, no stuck state and no repeated messages, and the log told the story clearly. The Test 5 failure is closed. One reporting wrinkle came out of it - see Milestone 2 in section 7.

9. ~~Does waking on input become annoying in practice?~~ **No**, and the disarm-on-manual-off rule did its job: after switching off by hand, typing did not take the headset back and the iPhone could claim it. But the tester asked for one change - **the mouse should not wake it**, because they do not use a mouse and can brush the trackpad by accident. Now design; see 4.1.

### Still open

6. **Does zeros still work after a much longer idle gap?** Testing used 20-30 second gaps. Real use involves far longer ones - an hour away from the desk, or a laptop that has slept. If clipping reappears after a long gap, fluctuate is the next thing to try. **Residual risk, watch for it in daily use.**

7. **Does the AeroClip behave the same on battery, or after a Windows sleep/resume cycle?** Sound Keeper carries explicit handling for modern-standby suspend/resume events, which suggests this bites in practice. Not yet tested.

### Resolved by Milestone 3 hardware testing (2026-09-05)

Raw results are in `test-milestone-3.txt`, Tests 11-18.

10. ~~Do the earcons sound right?~~ **Nearly.** The shape was right first time - "can you tell the on-tone from the off-tone without thinking about it? Yes", no clicks or distortion, nothing to change about how they sound. The numbers were not: volume wanted halving to 0.1, and the notes wanted shortening by about a fifth. Both done.

11. ~~Do the automatic earcons become a nuisance?~~ **Yes.** Heard "too often", "annoying", and the tester asked for the automatic release and re-wake to be silent. Done, and unconditionally rather than as a setting - see 4.4.

12. ~~Is the off-tone heard in full before the headset lets go?~~ **Yes,** every time, and the iPhone then took the headphones "pretty much immediately". The ordering design in 4.4 is confirmed. This was the question the milestone rested on.

13. ~~Does `Ctrl+Win+F12` clash with anything in JAWS?~~ **No.** Nothing stopped working, nothing broke that quitting fixed, and the tester is happy with the combination - while asking to be able to change it in the settings dialog.

### Resolved by the second Milestone 3 round (2026-09-05)

Raw results are in `test-milestone-3-round-2.txt`, Tests 19-23.

14. ~~Is the lead-in silence enough to stop the on-tone being clipped?~~ **Yes at 250 ms**, and the whole tone was audible. Now halved to 125 ms, because it then felt slow to arrive and the tester could hear JAWS begin to speak before the tone started - which means the link was already up well inside the old figure.

16. ~~Does `mouse on` actually work?~~ **Yes.** Confirmed for the first time, once the test stopped confounding it with a manual `off`.

Also settled, and worth recording because they were the whole point of the milestone: the earcons are **right** - volume, length, shape, no clicks - and automatic transitions are **silent**, with "I heard no tones, which was perfect."

### Resolved by the third Milestone 3 round (2026-09-05)

Raw results are in `test-milestone-3-round-3.txt`, Tests 24-26.

15. ~~Does the tray work from the keyboard?~~ **Yes, finally.** One icon, not
    two. JAWS reads `StableSound: headphones free` - once, correctly. `Enter`
    toggles keep-alive and toggles it exactly once per press. The context menu
    opens on the Applications key, arrows correctly, and both items work. The
    diagnosis was right: the shell **sends** its callback, and a sent message
    never comes back out of `GetMessage`. The tester's verdict: "everything
    worked beautifully and felt great to use."

    The duplicated name and the dead `Enter` were indeed ghost icons from
    killed processes - removing the icon on console close fixed both, and the
    reboot before the round cleared the existing corpses.

17. ~~Does the trackpad still wake keep-alive?~~ **It did, and the log finally
    said why.** Raw mouse input was the right mechanism and was working - 42
    events were correctly attributed to the mouse. The fault was a race, not a
    misidentification, and it is now fixed by holding the decision. See below
    and 4.2.

### What the log showed about the trackpad

Worth recording in full, because two rounds were lost to guessing at this from
the outside and the third round's diagnostics settled it in three lines:

```
12:02:06 input seen, no mouse activity ever recorded -> keyboard
12:02:06 woken by keyboard input
12:02:06 input seen, mouse active 0 ms before it -> mouse
```

Every event of a trackpad sweep **except the first** was attributed correctly.
The first is the one that wakes. `GetLastInputInfo` is stamped the instant the
input lands, while `WM_INPUT` has to be queued and dispatched to us afterwards;
polling in the gap between the two finds a pointer tick that is still stale, so
the leading event of every sweep read as a keypress. Once a sweep is under way
the pointer tick is never stale again, which is exactly why the fault looked
like a misidentification and survived two rounds of being reasoned about.

**How late is `WM_INPUT`, measured?** Both spurious wakes in the round are
followed within a single 100 ms engine tick by a pointer event aged 16 ms and
0 ms respectively. So the mouse reports itself in **under 100 ms**. The hold is
set to 250 ms, which is margin over a measured figure rather than another
guess, and the log now prints the real number as a negative pointer age
alongside the wait that caught it - so it can be tightened on evidence.

The three remaining wakes in the round had no pointer event behind them at all
(ages of 6 s, 56 s and 219 s). Those were the genuine keypresses of Tests 25.8
and 25.9, correctly handled then and untouched by the fix.

### Resolved by the Milestone 4 round (2026-09-05)

Raw results are in `test-milestone-4.txt`, Tests 27-32.

18. ~~Does holding the decision stop the trackpad waking keep-alive?~~ **No.**
    "Yes, it stayed off for a few swipes, but then turned keep-alive on", and a
    flick, a tap and a rest-and-lift all woke it. That is the third mechanism
    to fail, and the tester ended it: "Please give up on this feature now and
    remove all code that was there to facilitate it." Closed as abandoned, not
    as solved. See 4.1.

    The keyboard side was flawless again: never woke without a keypress, never
    failed to wake with one, and the added 250 ms was barely perceptible - "I
    noticed it, but only because you mentioned it. Very tiny difference."

19. ~~How does JAWS read the settings dialog?~~ **Correctly, and this was the
    question the milestone rested on.** "The tab key read all the controls
    well. The dialogue behaves like a model accessible app, well done!" The
    combo box choices read sensibly, `Alt` with an underlined letter jumped to
    the right control, the refusal message for a bad hotkey was read out and
    the focus came back to the offending field, and the toggle hotkey went on
    working while the dialog was open with no second dialog opening.

    Two things came out of it, both now built: a group heading that JAWS never
    announced (question 21), and one label that only made sense if you had
    heard the heading (fixed; see 4.6).

20. ~~Is a second global hotkey welcome, or one combination too many?~~ **One
    too many.** "It won't be used often enough to warrant a hotkey, so having
    it in the tray only is my current preference." Now an option, off by
    default. See 4.6.

### Resolved by the Milestone 5 round (2026-09-06)

Raw results are in `test-milestone-5.txt`, Tests 33-38.

21. ~~Can JAWS be made to announce the group headings?~~ **No, and it no longer
    matters.** "Show control group info" was already on - it is the JAWS
    default - and the headings are still not announced. The tester closed it:
    "due to the options being very clear, this does not matter... no further
    work on this is needed." Closed unsolved. The mitigation carried the whole
    weight instead and was confirmed to work: every label reads on its own.

22. ~~Does dropping the mouse handling cost anything in daily use?~~ **No.**
    Waking works, and the quarter second saved is below the threshold of
    noticing - "feels nearly the same. Happy with how it is." The trade is
    accepted: "No, its fine. I've learned to avoid touching it." And the part
    that mattered held - switching keep-alive off by hand keeps the headset
    free for the phone whatever is touched afterwards.

23. ~~Do the version 6 common controls change how the dialog reads?~~ **No,
    and that is the wanted answer.** "Yes, felt like the same good experience...
    Everything still worked the same. No issues while tabbing through."
    Mnemonics, combo boxes and per-control reading all unchanged.

### Resolved by the Milestone 5.1 round (2026-09-07)

Raw results are in `test-milestone-5-1.txt`, Tests 39-43. Every one passed.

24. ~~Do the five follow-ups behave under JAWS?~~ **Yes, all five.** The tray
    icon reads "StableSound Headphones free" and "StableSound Headphones
    awake", one phrase however many times the state changes. `Ctrl+Win+;` is
    accepted and read back, and `ctrl+win+:` reads back as `Ctrl+Win+Shift+;`
    with the folding understood rather than merely tolerated. The refusal
    message is reviewable with the arrow keys straight from where the focus
    lands. Logging is off and the settings hotkey is on, and both read
    correctly to someone meeting them for the first time.

25. ~~Is the message window better than the message box, or only different?~~
    **Better, for short messages too.** The offer to keep a message box for the
    one-line complaints was declined: "No, this is good as it is." So there is
    one path for every message, which is also the simpler thing to maintain.

### Still open after the Milestone 5.1 round

26. **Does the reworded message heading read well?** It now says what kind of
    message this is - "What went wrong, and how to fix it:" or "What
    happened:" - instead of describing the control, which is what the round
    asked for. Verified through UI Automation, not heard. Riding along with
    the Milestone 6 round as Test 47.

### Opened by Milestone 6, for its round

27. **Does the help document work as a document?** The whole reason it is HTML
    in a browser rather than a text file or a read-only edit is heading
    navigation - `H` between headings, `Insert+F6` for the list. That has been
    reasoned about and never heard. If it turns out not to help, the format
    was the wrong choice and a plain text file would have been simpler. Test
    46.

28. **Is `Exit StableSound` in a safe place?** It is last in tab order, after
    OK and Cancel, so tabbing one stop too far past the settings reaches OK
    rather than the button that quits. That is a judgement made without being
    able to feel it. Test 48 asks directly.

29. **Is anything about the console actually missed?** It was the diagnostic
    interface for four milestones and the tester was fluent in it. The
    settings dialog and the log are meant to cover everything it did. Test 51.

30. **Do the failure windows read well?** Three of them - a tray icon that
    cannot be created, a hotkey another program holds, settings that cannot be
    saved - and none can be provoked to order, so none has been heard. The
    hotkey one is the likeliest to be met in real use. Not scripted; it will
    have to be caught when it happens, which is the reason each one names the
    path or the combination involved rather than merely saying it failed.

### Noted, not a defect

**`Ctrl+Win+;` clashes with Leasey.** Found while testing that punctuation
hotkeys work at all, which they do - the tester used `Ctrl+Win+Shift+;`
instead and it worked, then went back to `Ctrl+Win+F12`. Nothing for the app to
do: a combination Windows will register can still be taken by something that
hooks the keyboard ahead of us. Worth a line in the README, and a reason the
hotkey field is right to accept anything rather than offer a list.

**JAWS announces the hotkey.** Pressing `Ctrl+Win+F12` makes JAWS say "control win f12" before the tone - "slightly annoying". This is JAWS echoing a command key, not anything StableSound does or can intercept; a global hotkey is delivered to us *after* the screen reader has already seen the keystroke. The only lever is JAWS' own "speak command keys" setting, which is global and probably not worth losing elsewhere. Worth re-checking against any hotkey the settings dialog offers, in case some combinations are echoed and others are not.

---

## 7. Plan

**Milestone 0 — toolchain. DONE (2026-08-29).** rustup + Rust 1.98.0, Visual Studio Build Tools 17.14.39 with the VC++ workload. Verified end to end: a real binary compiles, links and runs.

**Milestone 1 - spike. DONE (2026-08-30).** All three questions answered; see section 6 and `test-milestones-1-2.txt`. The tester reported the spike itself was comfortable to use with JAWS, so the line-based, transition-only output pattern is worth reusing for future diagnostic tools.

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

**Hardware test results (2026-08-30), raw notes in `test-milestones-1-2.txt`:**
- Test 4, speech postpones release: **passed.**
- Test 5, following a device change: **failed.** Disconnecting the headphones stopped the engine for good; the user had to type `on` again. Fixed - see below.
- Test 6, log readability: **passed** ("Yes, clear", nothing missing).
- Test 7, general use: **passed**, no clipping. Timeout preference: 30 s.

**Fixes applied after testing:**

1. **Device loss no longer kills the engine.** The cause of Test 5 was conflating "should keep-alive be running" with "is a stream open". They are now separate: losing a device clears the stream but leaves intent standing, so the engine retries every 2 s and reopens on whatever Windows switched to. Retry failures are logged once, not repeatedly. Bounded naturally by the release timeout, so a headset that never returns does not retry forever.
2. **Wake on input**, at the tester's suggestion. See section 4.1.
3. **Default idle timeout 60 s to 30 s**, at the tester's preference.
4. **Fixed a bug in the wake feature found by its own test.** The check short-circuited on the config flag, so while waking was off the input watcher never refreshed its baseline; switching it on then compared against a stale timestamp and fired a spurious wake immediately. The watcher is now polled unconditionally and the flag consulted afterwards.

**Second hardware round (2026-09-05), raw notes in `test-milestones-1-2.txt`:**
- Test 8, device-loss recovery: **passed.** Disconnect, wait, reconnect - it came back on its own. The Test 5 fix holds.
- Test 9, waking on input: **passed**, including the part that matters most - after a manual `off`, typing did not take the headset back and the iPhone could claim it.
- Test 10, living with it: **passed.** No clipping, 30 s better than 60 s, waking never got in the way.

**One reporting wrinkle, worth knowing about.** During Test 8 the tester saw no `MOVED` or `INTERRUPTED` line, yet recovery plainly worked and the log recorded the device switch. That is the console harness, not the engine: `main.rs` drains the event channel only just before it blocks on `lines.next()`, so an event that arrives while it is waiting for input is not printed until the next time Enter is pressed. The engine writes the log file directly from its own thread, which is why the log was complete and the console was not.

Not worth fixing in the harness - Milestone 3 makes state changes audible through earcons, which is the real answer, and the log already covers after-the-fact reading. Recorded because it will otherwise look like a bug the next time somebody reads a transcript.

**Change requested and carried forward:** waking on input should ignore the mouse (now design, see 4.1), and the idle timeout needs an easy way to change it (Milestone 4).

The console harness in `main.rs` was scaffolding for testing the engine. Milestone 3 adds the tray and hotkey **alongside** it rather than replacing it, so there is still a diagnostic interface while the GUI is unproven; Milestone 6 removes it.

**Milestone 3 - control surface. DONE (2026-09-05), three hardware rounds. Sound signed off in the second, tray in the third. The trackpad fix failed a fourth round in Milestone 4 and the feature was then dropped; question 18.**

Built: the global hotkey with its own parser and a refusal to register a bare key; earcons rendered into the keep-alive stream, with an anti-click envelope; the tray icon, its real Win32 menu and an icon drawn in code; the hidden window and message loop; and the keyboard-only wake change carried over from Milestone 2's round. Design detail in 4.4.

Verified automatically:
- Startup registers `Ctrl+Win+F12` on this machine, so nothing here is holding it.
- A full on/off cycle opens and releases the AeroClip cleanly, with no hang and no retry storm.
- Release latency measured at 125 ms with earcons off and 340 ms with them on, well inside the 1 s drain ceiling - so `drain` is genuinely draining rather than timing out.
- Config round-trips every new setting; a mistyped hotkey is reported rather than silently dropped.
- 24 unit tests. `cargo clippy --all-targets -- -D warnings` clean.
- Release binary **266 KB**, up from 248 KB, against the ~1 MB budget.

Two design points settled while building:

- **Earcons play on the existing keep-alive stream, before it is torn down**, and the queued-audio target dropped from 500 ms to 200 ms so the tone does not trail the keypress. See 4.4.
- **The console harness stays**, on its own stdin thread feeding the same command channel as the hotkey and tray.

**First hardware round (2026-09-05), raw notes in `test-milestone-3.txt`:**

- Test 11, does the hotkey work: **passed.** Worked from a word processor, Chrome and the desktop, and toggled exactly once when held - `MOD_NOREPEAT` doing its job.
- Test 12, are the tones right: **passed on shape, failed on numbers.** Direction obvious without thinking, no clicks or distortion. Too loud, about a fifth too long, and the start of the on-tone was missing.
- Test 13, off-tone before release: **passed,** and this was the one that mattered. Whole tone, both notes, every time, and the phone took the headphones straight afterwards.
- Test 14, JAWS clash: **passed.** Nothing broke. JAWS does announce the hotkey itself, which we cannot intercept - see section 6.
- Test 15, the tray under JAWS: **failed.** No menu from the Applications key or `Shift+F10`, and `Enter` on the icon did nothing. The tooltip was also "too verbose", and read stale.
- Test 16, mouse no longer wakes it: **failed.** A trackpad sweep still woke keep-alive with `mouse off` set.
- Test 17, living with it: **passed on the product, failed on the tones.** No clipping. The automatic tones were "annoying" and heard "too often".
- Test 18, the known silent-off gap: **passed,** confirmed harmless, including with no headphones connected at all.

**Fixes applied after the round:**

1. **Tones only for what the user does.** Automatic releases and the wake that follows are silent, unconditionally. Also makes an automatic release instant, since nothing has to drain. See 4.4.
2. **The trackpad fix that was not a trackpad fix.** `mouse off` failed because the finger *lift* at the end of a sweep is an input event arriving after the pointer has already stopped, which reads exactly like a keypress. The pointer now has to have been still for a 500 ms settling window. See 4.1. Covered by a unit test that replays the failing sequence.
3. **The tray rebuilt on `NIM_SETVERSION` / version 4.** The menu was never broken; the keyboard's messages were never being delivered, because the icon had not asked for the protocol that carries them. See 4.4.
4. **Tone numbers to the tester's figures.** Volume 0.2 to 0.1, notes shortened a fifth, 250 ms of lead-in silence in front of the on-tone, and the queued-audio target 200 ms to 150 ms for a snappier release.
5. **Tooltip cut down**, since it is the icon's accessible name and is read in full each time.

**Verified after the fixes:** 30 unit tests, `cargo clippy --all-targets -- -D warnings` clean, and a scripted silent on/off cycle at zero volume - manual release 260-350 ms with earcons, 120-190 ms without, no interruptions or retries over six cycles.

**Not verified, and cannot be from here:** everything audible, and the whole tray. Second round is `test-milestone-3-round-2.txt`, Tests 19-23.

**Second hardware round (2026-09-05), raw notes in `test-milestone-3-round-2.txt`:**

- Test 19, are the automatic tones gone: **passed.** "I heard no tones, which was perfect."
- Test 20, the tones themselves: **passed.** Volume right, length right, whole on-tone audible, no clicks, switching off quick enough. One refinement: the on-tone felt slow to start.
- Test 21, the tray from the keyboard: **failed again, identically.** No menu, `Enter` does nothing, name still read twice over.
- Test 22, mouse off and mouse on: **failed again.** A sweep and even a short flick still woke keep-alive. `mouse on` was confirmed working for the first time.
- Test 23, living with it: **passed.** No clipping, no unasked-for tones, clean handover to the phone.

**Fixes applied after the second round:**

1. **The tray callback is handled in the window procedure and re-posted.** The shell *sends* it, and `GetMessage` dispatches sent messages straight to the window procedure without ever returning them, so a loop-only design could never have seen a single click or keypress. Version 4 was accepted all along. Version 3 messages are now accepted too, so the protocol in force no longer matters. See 4.4.
2. **The mouse reports itself.** Raw mouse input replaces two failed attempts at inferring the source from the cursor. Mouse usage page only, payload never read, so the no-key-data property is intact. See 4.1.
3. **The icon is removed when the process is killed**, not only when it exits cleanly, so closing the console window stops leaving a ghost behind. Ghosts are the leading explanation for the doubled name, and a ghost is also unpressable, which would look exactly like the bug above.
4. **Every tray callback is logged**, and so is whether version 4 and raw input registered. Three rounds have now been spent inferring this from the outside.
5. **A `diag` setting** that logs the reasoning behind each keyboard-or-mouse decision, with the timings it rested on.
6. **Lead-in silence halved to 125 ms**, at the tester's suggestion.

**Verified locally this time, rather than reasoned about.** A test drives the running app from another process:
- `WM_TRAY` sent the way the shell sends it, carrying `NIN_KEYSELECT` and `NIN_SELECT`, toggles keep-alive. A doubled `Enter` toggles once.
- Version 4 is accepted, and raw mouse input registers.
- A synthesised mouse movement is classified as the mouse, not the keyboard.
- Cost of the raw input stream measured: ~75 us per event, about 1 % of one core at a trackpad's 125 Hz while moving, nothing when still.
- 30 unit tests, clippy clean, release binary 273 KB.

**Still not verifiable from here:** whether the icon the tester actually reaches with `Win+B` is ours, and whether a real trackpad behaves like synthesised input. Third round is `test-milestone-3-round-3.txt`, Tests 24-26.

**Third hardware round (2026-09-05), raw notes in `test-milestone-3-round-3.txt`:**

- Test 24, the tray: **passed, completely.** One icon, name read once and correctly, `Enter` toggles once per press, menu opens on the Applications key and arrows properly, both items work, clean exit and the icon goes. The log carried the `tray callback` lines that would have settled it either way, and they were there. "Everything worked beautifully and felt great to use."
- Test 25, the trackpad: **failed a third time - but the log said why**, which was the point of building it. See question 17. The keyboard side was flawless: never woke without a keypress, never failed to wake with one.
- Test 26, living with it: **passed.** No clipping, nothing unasked-for, and the shorter lead-in is good. Verdict: "keyboard perfect, trackpad needs fixing."

The round also produced one request: **a settings item in the tray menu**, which belongs with Milestone 4.

**Fix applied after the third round - the last one:**

**An input that no pointing device accounts for is held before it is judged.** Raw mouse input was never the problem; the two clocks were. `GetLastInputInfo` is stamped the instant input lands, `WM_INPUT` is queued and dispatched afterwards, and a poll landing between the two finds a stale pointer tick - so the *leading* event of every sweep read as a keypress, and the leading event is the one that wakes. Such an input now waits 250 ms and is judged when the wait is up, by which time the `WM_INPUT` explaining it has arrived. Input the mouse already accounts for is still decided on the spot. See 4.2.

The 250 ms is **measured, not guessed**: in the tester's own log both spurious wakes are followed within a single 100 ms engine tick by a pointer event aged 16 ms and 0 ms, putting the real gap under 100 ms. The log now records the figure directly - a negative pointer age next to the wait that caught it - so it can be tightened on evidence rather than reopened as a guess.

Cost: waking on the keyboard is 250 ms later than it was. That lands where it is affordable. Waking from a release means reconnecting Bluetooth, measured at around three seconds, so a quarter-second on the front of it is nothing; and when keep-alive is already on, the wake path only postpones the idle timer.

**Verified:** 38 unit tests, including the leading-event case that failed on hardware, a steady-typing case that would catch a hold which never expires, and the tick-counter wrap. `cargo clippy -- -D warnings` clean.

**Not verified:** the fix against a real trackpad. Question 18, folded into the Milestone 4 round.

**Milestone 4 - settings. DONE (2026-09-05), hardware round passed.**

Built: the settings dialog as a real Win32 dialog template; a second global hotkey that opens it; a `Settings...` item in the tray menu and a `settings` console command; "start when I sign in"; and runtime switching of the log. Design detail in 4.3.

**Three decisions worth keeping, because each one had a plausible alternative:**

1. **A dialog template, not `winsafe`.** See section 5. The short version: the template gives more accessibility than the constraint asks for, `winsafe` would fight the message loop this app already owns, and the whole dialog cost 22 KB.

2. **Modeless, not modal.** `DialogBoxParamW` would hand its answer straight back instead of over a channel, which is simpler. It also runs its own message loop, and that loop dispatches `WM_HOTKEY` to a window procedure that does not handle it - so the global hotkey would silently stop working for as long as the dialog was open. CLAUDE.md calls the hotkey the primary interface; that is not a trade worth making to save a channel. `IsDialogMessageW` in the existing loop gives the dialog its keyboard behaviour instead, and declines anything belonging to another window.

3. **The console's config is now shared, not copied.** The console harness owned its own `Config`. With a dialog writing settings too, that copy would drift the moment the dialog was used, and `status` would report settings that are not in force - the exact class of thing that has already cost this project two hardware rounds.

**Verified by driving the running app from another process:**

- The dialog is a real `#32770` with 29 controls. Every label sits immediately before the control it names, and every input carries `WS_TABSTOP`. Tab order is template order, so the reading order is the tab order.
- All settings populate from the config file correctly - device, signal, release mode, timeout, both hotkeys, all five checkboxes and the volume.
- OK writes them back, saves the file and reloads the engine. A changed hotkey is re-registered live, and the log records the change.
- A bare key with no modifier is refused: the message box explains, the dialog stays open, the focus returns to the offending field, and nothing is written.
- **The global hotkey toggles keep-alive twice while the dialog is open, and the dialog stays up.** This is the whole justification for decision 2 above.
- "Start when I sign in" reads 0 with no `Run` value and 1 with one, writes the quoted exe path, and removes it again.
- 41 unit tests, `cargo clippy --all-targets -- -D warnings` clean. Release binary **290 KB**, up from 268 KB, against the ~1 MB budget.

**The round answered the only question it was for: JAWS reads it correctly.** The tester: "The dialogue behaves like a model accessible app." Results below; raw notes in `test-milestone-4.txt`.

**Milestone 4 hardware round (2026-09-05), raw notes in `test-milestone-4.txt`:**

- Test 27, the trackpad one last time: **failed, for the third mechanism and
  the fourth round.** A sweep, a flick, a tap and a rest-and-lift all woke
  keep-alive. The keyboard side was flawless, and the 250 ms hold was barely
  perceptible: "I noticed it, but only because you mentioned it." Ended by the
  tester's request to drop the feature. See question 18.
- Test 28, reading the dialog: **passed, and this was the milestone.** "The tab
  key read all the controls well. The dialogue behaves like a model accessible
  app, well done!" Two follow-ups: the group headings were not announced, and
  "let go of them" made no sense heard in isolation.
- Test 29, changing something: **passed.** The timeout changed and took effect,
  the tray's `Settings...` item opened the dialog, Escape cancelled cleanly.
  The second hotkey was judged not worth its combination.
- Test 30, a bad hotkey: **passed.** The message was read out and the focus
  came back to the offending field. Asked for more in the message itself.
- Test 31, the hotkey while the dialog is open: **passed**, which is the whole
  justification for the dialog being modeless. It toggled with its tone, the
  dialog stayed up and still worked, and asking again raised the existing one
  rather than opening a second. "No, very good."
- Test 32, living with it: **passed.** No clipping, nothing unasked-for, and
  the dialog judged enough on its own to retire the console window.

**Milestone 5 - polish. DONE (2026-09-06), round passed.**

Entirely made of what the Milestone 4 round asked for. Design detail in 4.6,
and in 4.1 for the removal.

Built:

1. **The mouse-versus-keyboard feature removed**, at the tester's request after
   its fourth failed round. Out with it went the raw input registration and its
   system-wide `WM_INPUT` stream, the settling window, the two-clock race, the
   250 ms hold, `wake_on_mouse`, the `mouse` console command and the dialog's
   checkbox. Waking is immediate again.
2. **The settings hotkey made optional and off by default**, with the
   combination kept and offered so switching it on is one checkbox.
3. **A refused hotkey now names the mistake** - one of five - and is followed
   by one shared string listing every modifier spelling and key name, and
   saying that order, spacing and capitalisation do not matter.
4. **Every dialog label made to read on its own**, depending on neither its
   neighbours nor its group heading.
5. **Detailed logging given real content**: the peak meter crossing the audio
   threshold in each direction with the level, and how long each device took to
   open. It had lost its only content when the input handling went.
6. **An application manifest**, so the dialog is built from the same version 6
   common controls as every other dialog on the machine. There was none at all
   before.

Two defects found while checking the above, both fixed:

- **A UTF-8 byte order mark ate the config file's first setting.** The file is
  documented as safe to edit by hand and several Windows editors offer to add
  one. One ignored line among many that work is an unpleasant way to fail.
- **Five message strings had had their line continuations flattened** into runs
  of literal spaces, four of them long before this branch.

**Verified by driving the running app from another process:**

- The dialog is a real `#32770` of genuine `ComboBox`, `Edit` and `Button`
  controls. Every input carries `WS_TABSTOP`, every label sits immediately
  before the control it names, and each group's first control carries
  `WS_GROUP`. Read out of the live dialog, not out of the template.
- Every `GROUPBOX` rectangle encloses its own controls - the thing JAWS
  actually uses - which is how question 21 was narrowed down.
- The settings hotkey is **not** registered by default, and **is** registered
  when the config asks for it. Both confirmed from the log.
- Detailed logging adds `device opened in 30 ms` and, with a tone played
  through the default output, `audio detected, peak 0.4935` followed by
  `audio stopped, peak 0.0000; letting go in 4s unless something plays`. With
  it off, neither line appears.
- 35 unit tests, including one asserting that everything `Hotkey::parse`
  accepts is named in the advice string, so the two cannot drift apart.
- `cargo clippy --all-targets -- -D warnings` clean. Release binary **296 KB**,
  up from 290 KB, against the ~1 MB budget.

**The round passed** - `test-milestone-5.txt`, Tests 33-38. The version 6
controls changed nothing, the reworded labels read clearly on their own, each
refusal named the mistake actually made, and losing the mouse handling costs
nothing anyone can feel. The group headings could not be delivered and are now
closed as not needed.

**Milestone 5.1 - what that round asked for. DONE (2026-09-07), round
passed.** Design detail in 4.7. All five confirmed on the hardware, first
time: "all works really well and I'm very pleased."

1. **The tray icon reads its state once.** It is registered under the app's
   name and the tooltip carries the state alone, because Windows 11 composes a
   tray icon's accessible name as its registration label followed by its
   current tooltip.
2. **Logging off by default**, reversing the steer given in 4.6, at the
   tester's decision. Every test script from here on has to ask for it to be
   switched on first.
3. **The settings hotkey on by default**, reversing what the Milestone 4 round
   asked for, at the tester's decision.
4. **Punctuation keys allowed in hotkeys**, resolved through the keyboard
   layout rather than a table, with Shift folded in where a character needs it.
5. **Messages moved into a read-only edit** the arrow keys can review, keeping
   the system ding that says which kind of message it is.

And one thing built after the round, because the round asked for it:

6. **The message heading says what kind of message this is**, rather than how
   to read it - "What went wrong, and how to fix it:" or "What happened:".
   The old heading explained the arrow keys, and JAWS then said "read only
   edit", spending two announcements on one fact. Needs question 26 answering
   in the Milestone 6 round.

**Verified by driving the running app from another process:**

- The tray icon's UI Automation name - the string JAWS is handed - reads
  `StableSound Headphones free` and `StableSound Headphones awake`, off and on
  and off again, against the live notification area. Before the fix the same
  probe reproduced the reported doubling exactly.
- A refused hotkey opens a real `#32770` containing a real `EDIT` with
  `ES_MULTILINE` and `ES_READONLY`; the focus lands on it, all twenty lines fit
  without scrolling, Escape and Enter both dismiss it, and the focus returns to
  the field that was wrong.
- `hotkey ctrl+win+;` is accepted and reads back as `Ctrl+Win+;`;
  `ctrl+win+:` becomes `Ctrl+Win+Shift+;`, which is the same keys.
- 38 unit tests, `cargo clippy --all-targets -- -D warnings` clean. Release
  binary **297 KB**, up from 296 KB, against the ~1 MB budget.

- The reworded heading is what UI Automation gives as the focused edit's name,
  with the advice intact behind it - 817 characters over 20 lines, multi-line
  and read-only - and the focus returns to the offending field on dismissal.
  Adding a control ID to the static that carries it does not disturb the
  labelling.

**Round passed 2026-09-07.** Raw results in `test-milestone-5-1.txt`, Tests
39-43. Questions 24 and 25 closed; question 26 opened by the heading built
afterwards.

**Hard release. DEFERRED, probably not needed.** Soft release was confirmed sufficient on the AeroClip (see 2.4), so this is no longer planned work. Revisit only if another headset needs it, or if the 3-second handover becomes annoying. Reference if it ever happens: `m2jean/ToothTray`.

**Milestone 6 - ship. BUILT (2026-09-08), hardware round outstanding.**
Branch `m6-ship`, script `test-milestone-6.txt`, Tests 44-51.

Everything the milestone asked for is built: the console dropped and the
subsystem switched (6.1), a guard against a second copy (6.2), a test for the
recurring flattened-continuation defect (6.3), the help document and the three
dialog buttons (6.4), and the antivirus question researched and answered
(6.5). The README and LICENSE are written.

**The size-tuned release build needed nothing.** The profile has been
`opt-level = "z"`, LTO, one codegen unit, `panic = "abort"` and stripped
symbols since Milestone 0, and the binary is **294 KB** against a ~1 MB
budget - *smaller* than the 297 KB it started this milestone at, because
dropping the console gave back more than the help document and the three
buttons cost. The only thing left to try would be rebuilding the standard
library, which needs a nightly toolchain, and there is no reason to spend a
toolchain constraint on 700 KB of headroom.

Two things the Milestone 4 round confirmed belong here rather than earlier:

- **Drop the console harness** and switch to the windows subsystem. It was kept
  through Milestones 3 to 5 because it was the only diagnostic interface and
  the GUI was unproven. The GUI is proven now: "The dialogue is enough and I
  would be comfortable with the console window removed."
- **A help file, and a button that opens it** in both the settings dialog and
  the tray menu. Asked for, for "eventually". It needs the documentation to
  exist first, which is the README above.

Autostart is already built, in Milestone 4.

**Decisions taken at the start of the milestone**, answering the questions the
work raised before any of it was written:

1. **The help is HTML, opened in the browser.** Embedded in the exe and
   written out on demand, so the single-file constraint holds. A browser is
   the one place a JAWS user gets real heading navigation - `H` to move
   heading to heading, `Insert+F6` for a list of them - which a `.txt` in
   Notepad cannot offer and a read-only edit control cannot either. The
   message window built in Milestone 5.1 is right for twenty lines and wrong
   for a manual.
2. **The settings dialog gains Help, Open the log file, and Exit.** Dropping
   the console left `Exit` and `Open the log file` reachable only from the
   tray menu, and CLAUDE.md constraint 3 says the tray is never the only route
   to a feature.
3. **Antivirus: document it now, research signing and write it up.**
4. **Public repo, version stays 0.1.0.** A first public cut, not a 1.0 claim.

### 6.1 The console is gone (2026-09-07)

`#![windows_subsystem = "windows"]`, `console.rs` deleted, and with it the
stdin thread, `WM_CONSOLE_QUIT`, the `Win32_System_Console` feature, `Log`'s
console echo, and the `SetConsoleCtrlHandler` that removed the tray icon when
the console window was closed - a route that no longer exists.

**Where the output went.** Two ways, not one:

- **Routine things go to the log**: the settings in force at startup, the
  hotkeys claimed, the output devices available. The engine already wrote
  every state change to the log from its own thread - which is why the log was
  complete during Milestone 2's testing when the console was not - so the
  message loop no longer repeats it.
- **Failures open the message window**, the read-only edit built in Milestone
  5.1. Logging is off by default, so a log nobody switched on is not a report.
  Three cases: the tray icon failing to be created, a hotkey that could not be
  claimed, and settings that could not be saved. Each says what Windows said
  and what to do about it.

**One deliberate exception, and one deliberate silence.**

The exception is that a *device* that will not open stays in the log and never
opens a window. The engine raises it on every disconnection, retries on its
own and recovers - proven on hardware in Milestone 2's Test 8 - so a dialog
there would be precisely the message spam that round was checking for.

The silence is `config adjusted:`. The console said it out loud; now only the
log does. Adjustments happen when the settings file has been hand-edited out
of range, and interrupting every sign-in with a dialog is a poor trade for a
rare case whose corrected values the settings dialog already shows.

**Two things fell out of it that are worth recording**, because both were dead
weight the console had been holding up:

- **`Event` went from seven variants to two.** `Started`, `WokenByInput`,
  `Moved`, `Stopped`, `Interrupted`, `Substituted` and `Error` carried device
  names, signal names, stop reasons and error text - every one of them for the
  console to print, and every one of them already written to the log by the
  engine at the same moment it was sent. So the strings were being cloned
  across a channel to be dropped unread. It is now `KeepAliveOn` and
  `KeepAliveOff`, which is all the tray icon can act on, and the three
  non-transitions send nothing at all. The one payload worth keeping - that a
  stream reopened on a *different* device, which is the shape of both
  device-loss recovery and a default-output change - moved into the log line
  as `(moved)`.
- **`Command::On` and `Command::Off` went.** They existed for the console's
  `on` and `off`. The hotkey, the tray icon and the tray menu have only ever
  toggled.

### 6.2 One StableSound at a time (2026-09-07)

A problem the console used to hide. A second copy started while the first is
running puts a second icon in the notification area - which a screen reader
reads out exactly like the live one - fails to register the global hotkey
because the first copy holds it, opens a second keep-alive stream on the same
device, and competes to write the same log and settings files. While there was
a console, all of that announced itself in the window that had just appeared.
Now nothing would.

Nor is it far-fetched. "Start StableSound when I sign in" has been built since
Milestone 4, so the app is normally *already running* by the time the user
goes looking for it - and with no window of its own, the obvious way to check
is to run the exe again.

A named mutex, not `FindWindow` on the tray window class: two copies started
together can both look, both find nothing and both carry on, whereas a mutex
is decided by the kernel at creation. The name is `Local\`-prefixed, so it
scopes to the signed-in session rather than the machine - global hotkeys are
per-session, so two signed-in users are not in competition and should each get
their own StableSound.

The second copy says so and stops, in the message window, naming the
combinations **this machine** is set up with rather than the defaults - which
is the one reason the settings file is read before the mutex is claimed. A
silent exit was rejected: it cannot be told apart from the app failing to
start, and running the exe again is exactly what somebody does when they are
not sure whether it is running.

**Verified by running two copies:** the second opens a real `#32770` whose
focused control carries the whole message as its accessible name, naming
`Ctrl+Win+F12` and `Ctrl+Win+F11` from the live settings; dismissing it leaves
exactly one process; and killing the first outright - no unwinding, no `Drop` -
still releases the mutex, so a fresh copy starts normally.

### 6.3 The flattened line continuation, caught by a test this time

Milestone 5 found five message strings whose line continuations had been
flattened into runs of literal spaces, "four of them long before this branch".
Writing 6.2 produced a sixth, and this time the cause is known and written
down.

It happens when the backslash ending a line inside a string literal is lost.
Rust then keeps the newline *and* the next line's indentation, so a message
reading `one can run \` + newline + fourteen spaces + `at a time` becomes `one
can run              at a time`. It compiles. It reads correctly in the source.
Nothing but running the program shows it - and the person this program is for
cannot see runs of spaces, only hear whatever the screen reader makes of them.

`rustfmt` was suspected and cleared by experiment: it leaves continuations
alone. The culprit here was the editing tool, which treated the backslash as
its own line continuation and ate it.

So there is now a test that reads the source of the six files carrying
user-facing prose and fails on any interior run of six or more spaces outside
a comment. Six is the threshold because a flattened continuation carries a
whole line's indentation - thirteen or fourteen spaces in this codebase -
while the deliberate alignment that does exist, the columns in
`Config::summary` and the key names in `hotkey::HOW_TO_WRITE`, never exceeds
four. Testing a program's own source text from inside it is an odd thing to
do; six occurrences of an invisible defect in a program for a blind user earn
it.

### 6.4 The help, and the three buttons that reach it (2026-09-08)

`help.html` is compiled into the exe with `include_str!` and written out only
when somebody asks to read it. That keeps constraint 5 - one self-contained
file, nothing to copy alongside - and rewriting it on every request, rather
than only when missing, means the help can never be older than the program it
describes. It costs 19 KB in the binary and nothing at runtime.

**HTML rather than a text file or a read-only edit**, because a browser is the
only one of the three that gives JAWS real heading navigation: `H` moves
heading to heading, `Insert+F6` lists them all. The message window built in
Milestone 5.1 is right for twenty lines and wrong for a manual with a dozen
sections.

**A finding worth keeping: open the help as a `file:///` URL, never as a
path.** Handing `ShellExecuteW` the path itself put an **"Open with"
chooser** on screen instead of a browser. The machine's `.html` entry still
resolves through `htmlfile` to Internet Explorer, which Windows 11 does not
have; the modern default browser is recorded under `UserChoice` and is only
consulted for the *protocol*. Passing `file:///C:/.../stablesound-help.html`
goes through the protocol handler and opened Chrome correctly, titled
"StableSound Help".

The chooser is the more dangerous of the two failures, because `ShellExecuteW`
reports it as **success** - 42, comfortably above the 32 that separates its
error codes from its meaningless positive ones. It did launch something. So no
amount of checking the return value would have caught this; only running it
did. The return value is now checked as well, for the ordinary failures, and
both openers say where the file is if they cannot open it.

`docs.rs` holds the help and the log together, because they are the same
gesture from the user's side and Milestone 6 gave both of them two callers
instead of one. The log still opens by path - a log is a text file and belongs
in a text editor, not a browser.

**The dialog gained three buttons**, so that dropping the console did not
leave `Exit` and `Open the log file` reachable only from the tray menu, which
CLAUDE.md constraint 3 forbids:

- `Open the log &file`, inside the *Log file* group immediately after the two
  logging checkboxes, so it is met where the log is being thought about.
- `H&elp` and `E&xit StableSound` on the bottom row. `F1` opens the help too.
- Mnemonics had to work around the letters already taken: `E` for Help,
  `F` for the log, `X` for Exit. O S L A K P V H U T W D I were spoken for.

**Exit is last in template order**, after OK and Cancel, and placed away from
them on screen. Tab order is template order, so tabbing one stop too far past
the settings reaches OK, not the button that quits StableSound. It discards
anything typed but not accepted, exactly as Cancel does; the help says so. It
destroys the dialog and then calls `PostQuitMessage`, which is sound only
because a modeless dialog's procedure runs on the thread that owns the queue -
one more thing that would not work had the dialog been modal.

**Verified by driving the running dialog from another process:** 32 controls,
still in template order with every label immediately before the control it
names; the three new ones are real `Button` windows carrying `WS_TABSTOP` and
their mnemonics, with Exit enumerating last. The Help button and `F1` each
opened the written file in the default browser, with no "Open with" chooser.
The written file is byte-identical to `help.html`. `Open the log file` with no
log present says so, naming the checkbox to tick and the path the file will
appear at. Exit quit the process cleanly. 43 unit tests, three of them over
the path-to-URL conversion, including a space and a non-ASCII folder name.

**Not verified, and cannot be from here:** how any of it reads under JAWS,
and whether the help document is actually navigable by headings in practice.

### 6.5 The antivirus and SmartScreen problem (researched 2026-09-08)

A small unsigned binary that opens audio devices, registers global hotkeys and
can start itself at sign-in fits the profile heuristics dislike. The README and
the help now both say so plainly, describe the SmartScreen screen and how to
get past it, and say to download only from the releases page. That is the part
that is done and costs nothing.

Whether to sign it is a decision with a gate in front of it, and two facts
found while researching it change the shape of the answer.

**Fact one: an EV certificate no longer buys an instant SmartScreen pass.**
Microsoft removed that in March 2024. Reputation is now built by download
volume whatever the certificate, so signing does *not* make the "Windows
protected your PC" screen go away for the first users - which was the main
thing it used to be worth paying for. It still helps: a signed binary
accumulates reputation against the publisher identity rather than against each
new file hash, so the warning stops recurring after every release rather than
needing to be earned again. And a signature is what most antivirus heuristics
weigh most heavily.

**Fact two: the cheap option may not be available here.** Azure Trusted
Signing, renamed **Azure Artifact Signing** in 2026, is by far the cheapest
route at **$9.99/month** on the Basic tier - no hardware token, no key to look
after, short-lived certificates issued on demand. But **individual** sign-up is
limited to the USA and Canada. Organisations can use it from the USA, Canada,
the EU and the UK. The three-year identity history that public preview asked
for has been dropped.

So the first question is not "is it worth $120 a year" but **which of those two
categories this project falls into**, which depends on where the author is and
whether they are willing to sign up as a business entity rather than an
individual. That is a question for the author, not something to research
further.

**If Azure Artifact Signing is not available**, the fallback is an ordinary
Organisation Validated certificate from Sectigo or Comodo, around **$219 a
year** at the cheap end and $400 from DigiCert. Two costs beyond the money:
since June 2023 the private key must live on FIPS 140-2 Level 2 hardware, so a
USB token or a cloud HSM comes with it and has to be present at every build;
and since March 2026 certificates last at most 460 days, so this is a renewal
to diarise. Sole proprietors can be validated without a company, under the
CA/Browser Forum's sole-proprietor procedure.

**The free things, worth doing first either way**, and probably enough for an
app with this many users:

- Submit the binary to Microsoft as a false positive if Defender objects.
- Do the same with any other vendor that flags it; each has a form.
- Ship releases from one place, so what reputation does accumulate accumulates
  in one place rather than being scattered across re-uploads.

**Recommendation: ship 0.1.0 unsigned, with the README section, and revisit.**
Signing is a subscription and an ongoing obligation, its headline benefit was
withdrawn two years ago, and nothing about it is easier to do later than now.
If SmartScreen or an antivirus actually turns out to be stopping people using
it, that is the moment to spend the money - and by then it will be known which
of the two routes is open.

**Verified:** release binary **275 KB**, down from 297 KB. The PE subsystem
field reads 2, `IMAGE_SUBSYSTEM_WINDOWS_GUI`. The exe starts, stays up with no
console window and no visible window of its own, and no `conhost` is spawned.
38 unit tests, `cargo clippy --all-targets -- -D warnings` clean.

**Not verified, and cannot be from here:** everything audible, everything JAWS
reads, and the three failure windows, which need a combination to be taken by
another program to provoke.

**Current position (2026-09-08):** Milestones 0 to 5.1 complete and
hardware-tested, with nothing outstanding behind them, and Milestone 6 built
and waiting on the round that closes it.

That round is the last one. It carries question 26 from 5.1 and questions 27
to 30 from Milestone 6, and if it passes there is nothing between here and a
first release: "this will be a great first version."
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
