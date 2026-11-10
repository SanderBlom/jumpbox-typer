#!/usr/bin/env sh
set -eu

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

test_verification_runs_complete_feedback_loop() {
  test_tmp=$(mktemp -d)
  trap 'rm -rf "$test_tmp"' EXIT HUP INT TERM

  project="$test_tmp/project"
  fake_bin="$test_tmp/bin"
  command_log="$test_tmp/commands.log"
  mkdir -p "$project/tests" "$fake_bin"
  cp scripts/verify.sh "$project/verify.sh"

  printf '%s\n' \
    '#!/usr/bin/env sh' \
    'printf "cargo %s\n" "$*" >>"$JUMPBOX_COMMAND_LOG"' >"$fake_bin/cargo"
  printf '%s\n' \
    '#!/usr/bin/env sh' \
    'echo packaging >>"$JUMPBOX_COMMAND_LOG"' >"$project/tests/packaging.sh"
  printf '%s\n' \
    '#!/usr/bin/env sh' \
    'echo verification-tests >>"$JUMPBOX_COMMAND_LOG"' >"$project/tests/verification.sh"
  printf '%s\n' \
    '#!/usr/bin/env sh' \
    'echo build >>"$JUMPBOX_COMMAND_LOG"' >"$project/build.sh"
  chmod +x "$fake_bin/cargo" "$project/verify.sh" "$project/tests/packaging.sh" \
    "$project/tests/verification.sh" "$project/build.sh"

  (
    cd "$project"
    PATH="$fake_bin:$PATH" JUMPBOX_COMMAND_LOG="$command_log" ./verify.sh
  )

  expected="$test_tmp/expected.log"
  printf '%s\n' \
    'cargo fmt --all -- --check' \
    'cargo test --all-targets' \
    'cargo check --all-targets' \
    'cargo clippy --all-targets -- -D warnings' \
    'verification-tests' \
    'packaging' \
    'build' >"$expected"
  cmp -s "$expected" "$command_log" || fail "verification omitted or reordered checks"
}

test_ci_covers_supported_hosts() {
  workflow=.github/workflows/verify.yml

  [ -f "$workflow" ] || fail "verification workflow is missing"
  grep -F 'ubuntu-24.04' "$workflow" >/dev/null || fail "Linux CI host is missing"
  grep -F 'macos-15' "$workflow" >/dev/null || fail "macOS 15 CI host is missing"
  grep -F 'runs-on: macos-15' .github/workflows/release-packages.yml >/dev/null || \
    fail "macOS release packaging must use a supported Apple Silicon runner"
  if grep -F 'macos-14' "$workflow" .github/workflows/release-packages.yml >/dev/null; then
    fail "workflows must not use the retired macOS 14 runner"
  fi
  grep -F 'branches: [main]' "$workflow" >/dev/null || \
    fail "push verification should only run on main"
  grep -F 'cancel-in-progress: true' "$workflow" >/dev/null || \
    fail "CI does not cancel superseded verification runs"
  grep -F 'test "$(uname -m)" = arm64' "$workflow" >/dev/null || \
    fail "macOS CI does not enforce the supported Apple Silicon architecture"
  grep -F './scripts/verify.sh' "$workflow" >/dev/null || \
    fail "CI does not use the reproducible verification entrypoint"
  grep -F './scripts/with-headless-wayland.sh ./scripts/verify.sh' "$workflow" >/dev/null || \
    fail "Linux CI must exercise the Wayland backend"
  if grep -E 'xvfb|GDK_BACKEND: x11' "$workflow" >/dev/null; then
    fail "Linux CI must not substitute X11 for Wayland"
  fi
}

test_wayland_runner_propagates_failures_and_cleans_up() {
  test_tmp=$(mktemp -d)
  trap 'rm -rf "$test_tmp"' EXIT HUP INT TERM
  fake_bin="$test_tmp/bin"
  mkdir -p "$fake_bin" "$test_tmp/runtime"
  printf '%s\n' \
    '#!/usr/bin/env sh' \
    '[ "$1" = -- ] || exit 99' \
    'shift' \
    'exec "$@"' >"$fake_bin/dbus-run-session"
  # Emulate older Cage versions that ignore the application's exit status.
  printf '%s\n' \
    '#!/usr/bin/env sh' \
    '[ "$1" = -- ] || exit 99' \
    'shift' \
    '"$@" || true' >"$fake_bin/cage"
  chmod +x "$fake_bin/dbus-run-session" "$fake_bin/cage"

  PATH="$fake_bin:$PATH" TMPDIR="$test_tmp/runtime" \
    ./scripts/with-headless-wayland.sh sh -c '
      [ "$GDK_BACKEND" = wayland ] &&
      [ "$GSK_RENDERER" = cairo ] &&
      [ "$WLR_BACKENDS" = headless ] &&
      [ "$WLR_RENDERER" = pixman ] &&
      [ "$JUMPBOX_REQUIRE_WAYLAND" = 1 ] &&
      [ -z "${DISPLAY:-}" ] &&
      [ -d "$XDG_RUNTIME_DIR" ]
    ' || fail "Wayland runner did not configure an isolated software display"

  result=0
  PATH="$fake_bin:$PATH" TMPDIR="$test_tmp/runtime" \
    ./scripts/with-headless-wayland.sh sh -c 'exit 7' || result=$?
  [ "$result" -eq 7 ] || fail "Wayland runner swallowed command failure"

  printf '%s\n' '#!/usr/bin/env sh' 'exit 0' >"$fake_bin/cage"
  if PATH="$fake_bin:$PATH" TMPDIR="$test_tmp/runtime" \
    ./scripts/with-headless-wayland.sh true 2>"$test_tmp/error.log"; then
    fail "Wayland runner accepted an unexecuted command"
  fi
  grep -F 'did not complete' "$test_tmp/error.log" >/dev/null || \
    fail "Wayland startup failure did not explain the missing test run"
  [ -z "$(ls -A "$test_tmp/runtime")" ] || fail "Wayland runner leaked runtime directories"
}

test_verification_runs_complete_feedback_loop
test_ci_covers_supported_hosts
test_wayland_runner_propagates_failures_and_cleans_up
echo "verification tests passed"
