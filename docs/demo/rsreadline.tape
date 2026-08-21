# Demo recording for rsreadline. Regenerate with:
#
#   VHS_NO_SANDBOX=true vhs docs/demo/rsreadline.tape
#
# (VHS_NO_SANDBOX works around Chrome's sandbox needing kernel features
# some containers/CI runners don't have; drop it if yours doesn't.)
#
# The Hide block below sources setup-demo-env.sh, which builds an isolated
# HOME with a synthetic .bash_history (docs/demo/fake_bash_history) and a
# scratch git repo so the recording never shows the operator's real
# username or history. See that script's comments for why plain Delete
# stands in for Shift+Delete here.
#
# When rsreadline grows a new capability worth showing, add a beat to the
# Show block below rather than starting a new tape.

Output docs/demo/rsreadline.gif

Require cargo

Set Shell "bash"
Set FontSize 20
Set Width 1000
Set Height 500
Set Theme "Dracula"
Set Padding 20
Set TypingSpeed 120ms

Hide
Type "source docs/demo/setup-demo-env.sh"
Enter
Show

# Typing shows matching history entries below the prompt.
Sleep 1000ms
Type "git"
Sleep 700ms

# Arrow keys select a suggestion, highlighted in reverse video; cycle a
# few times so the selection visibly moves, not just the top match.
Down
Sleep 700ms
Down
Sleep 700ms
Down
Sleep 1000ms

# Enter accepts the selection and runs it.
Enter
Sleep 1500ms

# Cycling through more matches without running anything.
Type "docker"
Sleep 1200ms
Down
Sleep 700ms
Down
Sleep 1200ms
Ctrl+C
Sleep 800ms

# An on-screen caption explaining the shortcut before demonstrating it
# (typed as a bash comment, so it's a no-op when it runs).
Type@30ms "# Shift+Delete removes suggestion from history"
Sleep 800ms
Enter
Sleep 1200ms

# A typo'd command sits in history like any other entry...
Type "git sta"
Sleep 1500ms
Down
Sleep 700ms

# Cycle down to the typo specifically (top match is "git stash").
Down
Sleep 1000ms

# ...Delete removes it for good (bound to Shift+Delete in real usage --
# see setup-demo-env.sh).
Delete
Sleep 1000ms
Ctrl+C
Sleep 600ms

# Retyping it proves it's gone: no more suggestion.
Type "git stage"
Sleep 1800ms
Ctrl+C
Sleep 1s
