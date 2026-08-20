# Demo recording

`rsreadline.gif` is generated from `rsreadline.tape` via
[VHS](https://github.com/charmbracelet/vhs), which drives an isolated bash
session and records it — no manual typing, and no real username or
`.bash_history` ever shows up in the recording.

## Regenerate

```sh
sudo apt install vhs ttyd ffmpeg   # or the Homebrew/Nix equivalents
VHS_NO_SANDBOX=true vhs docs/demo/rsreadline.tape
```

`VHS_NO_SANDBOX` works around Chrome's sandbox needing kernel features some
containers/CI runners don't have (VHS records through a headless Chrome
screenshotting an xterm.js terminal); drop it if your environment doesn't
need it.

## How it works

- `setup-demo-env.sh` is sourced (not executed) from the tape's `Hide`
  block. It builds `rsreadline` if needed, creates a scratch `HOME` with a
  synthetic `.bash_history` (`fake_bash_history`) and a throwaway git repo,
  sets a generic `PS1`, and evals `rsreadline init bash`.
- `fake_bash_history` is the seed data the demo suggestions are drawn from.
  Edit it to change what shows up.
- The real "remove from history" shortcut is Shift+Delete, but VHS's
  tape language can't synthesize that exact key combo (see the source
  comment in `setup-demo-env.sh`). The setup script binds plain Delete to
  the same handler for this recording only, and the tape uses `Delete`.

## Updating the demo

When rsreadline gains a capability worth showing, add a beat to the `Show`
block in `rsreadline.tape` rather than writing a new tape — keep it one
script per feature set, not one per version.
