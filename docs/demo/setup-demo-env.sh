# Source (don't execute) from rsreadline.tape's Hide block to build a
# throwaway demo environment: isolated HOME, seeded .bash_history, a scratch
# git repo, and a generic prompt, so recordings never show the operator's
# real username, hostname, or command history.

__rsreadline_demo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
__rsreadline_repo_root="$(cd "$__rsreadline_demo_dir/../.." && pwd)"
__rsreadline_bin="$__rsreadline_repo_root/target/release/rsreadline"

if [ ! -x "$__rsreadline_bin" ]; then
    (cd "$__rsreadline_repo_root" && cargo build --release --quiet)
fi

DEMO_HOME="$(mktemp -d)"
cp "$__rsreadline_demo_dir/fake_bash_history" "$DEMO_HOME/.bash_history"

git -C "$DEMO_HOME" init -q
git -C "$DEMO_HOME" config user.email "demo@example.com"
git -C "$DEMO_HOME" config user.name "demo"
git -C "$DEMO_HOME" commit -q --allow-empty -m "chore: initial commit"
git -C "$DEMO_HOME" checkout -q -b feature/suggestions

export HOME="$DEMO_HOME"
cd "$HOME" || return
export PS1='demo:~$ '

eval "$("$__rsreadline_bin" init bash)"

# The real shortcut is Shift+Delete, but VHS drives the terminal through a
# headless browser and can't synthesize that exact key combo. Alias the
# plain Delete key to the same handler for this recording only.
bind -x '"\e[3~": __rsreadline_delete_selected'

unset __rsreadline_demo_dir __rsreadline_repo_root __rsreadline_bin
clear
