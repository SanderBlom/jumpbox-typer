#!/usr/bin/env sh
set -eu

# Run a command on a private, software-rendered Wayland display and session bus.
# No X server, GPU, login session, or keyboard-event permissions are required.
if [ "$#" -eq 0 ]; then
  echo "Usage: $0 COMMAND [ARG ...]" >&2
  exit 2
fi

runtime=$(mktemp -d "${TMPDIR:-/tmp}/jumpbox-wayland.XXXXXX")
trap 'rm -rf "$runtime"' EXIT
trap 'exit 1' HUP INT TERM

export XDG_RUNTIME_DIR="$runtime"
export GDK_BACKEND=wayland
export GSK_RENDERER=cairo
export JUMPBOX_REQUIRE_WAYLAND=1
export WLR_BACKENDS=headless
export WLR_RENDERER=pixman
export WLR_LIBINPUT_NO_DEVICES=1
export JUMPBOX_WAYLAND_RESULT="$runtime/result"
unset DISPLAY WAYLAND_DISPLAY

# Cage creates a virtual seat and focuses mapped windows even without physical
# input devices. It starts the command only after its Wayland display is ready.
# Older Cage versions do not propagate the child's status, so record it explicitly.
dbus-run-session -- cage -- sh -c '
  "$@"
  result=$?
  printf "%s\n" "$result" >"$JUMPBOX_WAYLAND_RESULT"
  exit "$result"
' sh "$@" || {
  echo "Headless Wayland session failed." >&2
  exit 1
}

if [ ! -f "$JUMPBOX_WAYLAND_RESULT" ]; then
  echo "Wayland test command did not complete." >&2
  exit 1
fi
exit "$(cat "$JUMPBOX_WAYLAND_RESULT")"
