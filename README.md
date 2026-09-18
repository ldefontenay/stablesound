# StableSound

Keeps Bluetooth headphones awake so screen reader speech is not cut off at the
start, and lets go of them again so your phone can take over.

A single Windows executable, about 316 KB. No installer, no runtime to install,
and nothing outside its own folder unless you ask it to start when you sign in.

## Download

Two files, always the newest version, direct. They contain exactly the same
program. Take the zip if you use Microsoft Edge, and the exe otherwise.

**[Download stablesound.zip][zip]** — for Edge, or any browser that objects to
a downloaded program.

**[Download stablesound.exe][latest]** — the program on its own, nothing to
unzip.

[zip]: https://github.com/ldefontenay/stablesound/releases/latest/download/stablesound.zip
[latest]: https://github.com/ldefontenay/stablesound/releases/latest/download/stablesound.exe

The zip exists because Edge will not keep a downloaded unsigned program unless
you open its downloads flyout, find *Keep*, and confirm a second time — fiddly
with a screen reader, and alarming if you have not met it before. Edge has no
such objection to a zip; other browsers are less awkward about the exe. The zip
holds one file and nothing else, so extracting it gives you `stablesound.exe`.

That is the whole program. Put it anywhere you keep small programs and run it.
There is nothing to install and nothing to uninstall.

The [releases page][releases] has both files with their release notes, and
every earlier version. Please read [Antivirus and SmartScreen](#antivirus-and-smartscreen)
below before the first run — the program is unsigned, and Windows will say so
once, whichever file you took.

[releases]: https://github.com/ldefontenay/stablesound/releases

Windows 10 or 11, 64-bit. Requires no administrator rights.

## The problem

Bluetooth headphones switch their receiver off after a few seconds with nothing
playing, to save battery. The next time your screen reader speaks, the first
fraction of a second is lost while the headset wakes up. On a short
announcement that can be the whole word.

The usual fix is to stream something inaudible so the headset never idles. The
usual cost is that nothing else can then have the headphones — which matters a
great deal if they are also paired to a phone.

StableSound does the first without the second. It streams digital silence while
you are using the machine, and releases the device after a set quiet period, so
the headset becomes available to your phone again within a few seconds.

## Using it

Run `stablesound.exe`. It has no window; it puts an icon in the system tray
and waits.

- `Ctrl+Win+F12` — switch keep-alive on and off.
- `Ctrl+Win+F11` — open the settings.
- `Windows+B`, then the arrow keys, reaches the tray icon. `Enter` toggles
  keep-alive, and the `Applications` key opens its menu.

Both combinations can be changed, and the settings one can be turned off if you
would rather not spend a hotkey on it.

By default it releases the headphones after 30 seconds with nothing playing,
and starts again when you touch the keyboard. Switching off by hand always
sticks, so your phone keeps the headset until you ask for it back.

It also starts the way you left it: switched on when you last shut down means
switched on at your next sign-in, silently. With "start when I sign in" ticked
there is nothing left to press.

Press `F1` in the settings, or choose Help from the system tray menu, for
the full guide — every setting, and what to do when something goes wrong.

## Accessibility

This is the point of the project, not a feature of it. StableSound was built
with and for a JAWS user, and every part of it has been tested by one.

- The settings are a **real Win32 dialog** built from a resource template, so
  the controls are genuine `EDIT`, `COMBOBOX` and `BUTTON` windows that a
  screen reader already knows how to read. No drawn-UI toolkit is used, and
  none will be: they reconstruct an accessibility tree rather than exposing
  real controls.
- **Nothing is reachable only through the tray menu.** The global hotkey is the
  primary interface, and every function has a second route.
- **State changes are audible.** A rising tone when keep-alive comes on, a
  falling one when it goes off, played through the headphones being kept awake.
  Automatic changes are silent, because testing found automatic tones were
  heard far too often to live with.
- Messages appear in a window whose text can be reviewed with the arrow keys,
  rather than a message box that speaks once and cannot be gone back over.
- **It is awake before your screen reader speaks.** "Start when I sign in" asks
  Windows for a task that runs at the sign-in itself, not an entry in the
  startup list that waits for the desktop to settle. The difference was
  measured on hardware: about thirty seconds and a clipped first word, against
  speech that comes through whole.

## Antivirus and SmartScreen

StableSound is not signed with a code-signing certificate. The first time you
run it, Windows SmartScreen may show **Windows protected your PC**; *More info*
then *Run anyway* gets past it, once per machine.

Defender may then say it is running a cloud scan, and that this could take
about ten seconds. It passes, and StableSound starts normally — but nothing
announces the end of the scan, so with a screen reader those ten seconds are
just silence, and there is no way to tell a program that is starting from one
that was blocked. Don't wait for a sound that isn't coming: press
`Ctrl+Win+F12`. A rising tone means it is running and keep-alive just came on;
press it again for the falling tone to put it back. Either tone is proof.
`Windows+B` and the arrow keys will also find the tray icon if it is there.

Microsoft Edge objects earlier, at the download itself, and will not keep an
unsigned program without the two-step *Keep* described under
[Download](#download). The [zip][zip] avoids that, because Edge does not treat
a zip the same way. It does not avoid the SmartScreen prompt above: Windows
carries the mark through extraction, so that one still happens once.

An antivirus program may also object. A small unsigned program that opens audio
devices, registers global hotkeys and can start itself at sign-in matches the
shape of things worth being suspicious of, and heuristics cannot tell why it is
doing them. If yours quarantines it, its own interface will have a way to
report a false positive.

Both are reasonable defences behaving as designed. Download StableSound only
from [this repository's releases page][releases].

## What has actually been tested

Honest scope, because much of this can only be checked on real hardware:

- **Headset:** Anker Soundcore AeroClip, multipoint-paired to a laptop and an
  iPhone. Releasing the audio stream frees it for the phone in about three
  seconds, and pure digital silence is enough to stop the clipping — no
  inaudible tone needed.
- **Screen reader:** JAWS, on Windows 11.
- **Starting at sign-in**, by scheduled task, with the headphones already held
  and nothing clipped by the time JAWS speaks.
- **Both downloads, in Edge with JAWS.** The zip arrives with no objection at
  all; the exe is stopped with "not commonly downloaded" and has to be kept by
  hand. SmartScreen still appears on the extracted exe, followed by a Defender
  cloud scan that ends without a sound.
- **Not tested:** other headsets, other screen readers, Windows 10, behaviour
  across sleep and resume, and behaviour on battery.

If StableSound does not work on your headset, the settings offer two stronger
keep-alive signals before you give up on it.

## Files

Only `stablesound.exe` is distributed, on its own or inside `stablesound.zip`
with nothing else in it. Everything else it needs — the help, the
dialog template, the icon — is compiled into it. What appears beside it appears
because the program wrote it, in that folder or in `%APPDATA%\StableSound` if
that one cannot be written to:

- `stablesound.conf` — the settings. Safe to edit by hand.
- `stablesound.log` — only if you turn logging on, which is off by default.
- `stablesound-help.html` — written out each time you open the help, so it can
  never be out of date with the program that wrote it.

Deleting the executable and those three files removes StableSound completely.
The one exception is "start when I sign in", which asks Windows for a scheduled
task named `StableSound` that runs at logon — or, if Windows refuses it, for an
entry under `HKCU\...\CurrentVersion\Run` instead. It removes whichever it made
when you untick the box.

The task is preferred because it runs at the sign-in itself, while `Run` is not
reached until the desktop is ready and every other startup program has had its
turn — about half a minute on the machine this was measured on, which is half a
minute of clipped speech. Registering it needs no administrator rights, and the
tickbox reports whichever of the two is actually in force rather than what it
last tried to do. Move the exe somewhere else and the box reads unticked, which
is the truth; ticking it again repoints the task at where the program now is.

## Building

Rust stable-msvc and the Visual Studio Build Tools with the C++ workload.

```
cargo build --release
```

`build.rs` runs `rc.exe` over `stablesound.rc`, which compiles the dialog
template, the version resource and the application manifest into the binary.
`embed-resource` is the only dependency that is not the `windows` crate, and it
is build-time only — nothing of it ships.

```
cargo test                      # unit tests
cargo clippy -- -D warnings     # kept clean
```

Two tests are `#[ignore]`d because they touch the machine they run on rather
than only the code: one registers a real scheduled task and removes it again,
the other reports what this machine is currently set up with. `cargo test --
--ignored` runs them.

`PLAN.md` is the design record: what was tried, what failed on hardware, and
why each decision went the way it did. It is worth reading before changing
anything. The `test-milestone-*.txt` files are the raw hardware results behind
it, kept because they record behaviour that cannot be reproduced from code.

## Known issues

- **A combination can be taken silently.** Windows refuses a hotkey another
  program already holds, and StableSound says so. But a program that hooks the
  keyboard can intercept a combination *before* Windows offers it to anyone,
  and then nothing is refused and nothing is reported. `Ctrl+Win+;` and Leasey
  are a known case. If a combination does nothing and no message appeared, try
  another.
- **JAWS announces the hotkey before it acts.** A global hotkey reaches
  StableSound only after the screen reader has already seen the keystroke, so
  this cannot be intercepted. JAWS' own "speak command keys" setting is the
  only lever, and it is global.

## Licence

MIT. See [LICENSE](LICENSE).
