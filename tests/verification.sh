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
  grep -F 'macos-14' "$workflow" >/dev/null || fail "macOS 14 CI host is missing"
  grep -F 'test "$(uname -m)" = arm64' "$workflow" >/dev/null || \
    fail "macOS CI does not enforce the supported Apple Silicon architecture"
  grep -F './scripts/verify.sh' "$workflow" >/dev/null || \
    fail "CI does not use the reproducible verification entrypoint"
}

test_verification_runs_complete_feedback_loop
test_ci_covers_supported_hosts
echo "verification tests passed"
