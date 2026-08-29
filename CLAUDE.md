# StableSound

A Windows tray utility that keeps Bluetooth headphones awake so screen reader speech isn't clipped,
with a global hotkey to toggle it and an auto-release timer so the headset frees up for other devices.

**`PLAN.md` is the source of truth** for research findings, design decisions and the milestone plan.
Read it at the start of a session before proposing work.

## Non-negotiable constraints

These come from the purpose of the app and must not be traded away for convenience:

1. **Accessibility is the product.** The primary user is blind and uses JAWS. Every feature must be
   fully usable without sight.
2. **Real Win32 controls only.** Use `winsafe`. Never introduce a drawn-UI toolkit (egui, iced,
   Slint, Tauri, Electron) — they reconstruct an accessibility tree rather than exposing real
   controls, and behave worse under JAWS.
3. **Keyboard-first.** Every function reachable without the tray menu. The global hotkey is the
   primary interface. Tray menus are a convenience, never the only route to a feature.
4. **State changes are audible.** Earcons, not visual-only feedback.
5. **Stay small and portable.** Single self-contained `.exe`, no installer, no runtime dependency.
   Target under ~1 MB. Weigh every new dependency against that.

## Environment

- Rust stable-msvc (`rustc` 1.98+), Visual Studio Build Tools with the VC++ workload
- Windows 11
- Test hardware: Anker Soundcore AeroClip, multipoint-paired to this laptop and an iPhone

## Commands

```
cargo build                 # debug build
cargo run                   # run locally
cargo build --release       # size-tuned release build
cargo fmt                   # format
cargo clippy -- -D warnings # lint; keep clean
cargo test                  # tests
```

## Session hygiene

**Starting.** Read `PLAN.md`. Run `git status` and `git log --oneline -5` to see where things stand.
If the working tree is dirty, sort that out before starting new work.

**During.**
- Work on a branch per milestone: `m2-core-engine`, `m4-settings-dialog`. Docs-only changes can go
  straight to `main`.
- Commit in small, coherent steps with a clear message. Don't batch unrelated changes.
- Run `cargo fmt` and `cargo clippy` before each commit.
- When an open question in `PLAN.md` gets answered, update `PLAN.md` in the same commit as the code
  that answered it. The doc going stale is the main failure mode for this project.
- Record findings that contradict the plan, even when inconvenient. The design rests on assumptions
  that are explicitly untested.

**Finishing.**
- Update the milestone status in `PLAN.md`.
- Commit outstanding work. Never leave the tree dirty at session end.
- **Ask before pushing.** Don't push or open PRs unprompted.

**Never.**
- Commit secrets, tokens, or signing keys.
- Commit build artefacts (`/target`, `.exe`) — releases go through GitHub Releases.
- Claim something works on the AeroClip without actually testing it on the hardware. Say plainly when
  something is untested.

## Testing note

Much of this project can only be validated on real hardware — whether the headset stays awake,
whether it releases to the phone, how long reconnect takes. Automated tests can't cover that. When a
change touches keep-alive or release behaviour, say explicitly what needs a manual hardware check.

**Manual tests go in `test.txt`, never in chat.** Write the steps to the file with explicit
`ANSWER:` lines for the user to fill in, then read the file back. The user is blind and works with
JAWS; a file can be navigated and annotated at their own pace, whereas multi-step instructions in
terminal scrollback have to be re-read while performing them. Keep it plain text — no Markdown
tables or box drawing — put each instruction immediately before its answer field, and include an
"ANYTHING ODD:" catch-all per section.

Keep completed `test.txt` results in the repo. They're a record of hardware behaviour that can't be
reproduced from code, and the findings should be folded into `PLAN.md` once read.
