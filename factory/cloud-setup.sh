#!/bin/sh
# SessionStart hook (.claude/settings.json): readies an Anthropic cloud container for watane's tests.
# Does nothing on the Mac. Runs at every session start: each step skips itself when its tool is
# present, prints nothing, and the hook always exits 0.
[ "${CLAUDE_CODE_REMOTE:-}" = true ] || exit 0
cd "${CLAUDE_PROJECT_DIR:-.}" || exit 0
rustup target add wasm32-wasip2 >/dev/null 2>&1
# rustfmt.toml uses nightly-only options (Google Rust style); check.sh runs `cargo +nightly fmt`.
rustup toolchain install nightly --profile minimal --component rustfmt >/dev/null 2>&1
[ -d dev/allowlist/vendor ] || dev/scripts/vendor.sh >/dev/null 2>&1

# Linux packages, only those missing. The WebKitGTK/GTK pair is CI's ubuntu line
# (.github/workflows/ci.yml; app's gdk-sys needs gdk-3.0 in pkg-config). The X ones are
# app/linux.Dockerfile's for the driver (window) tests, which app/scripts/linux-test.sh runs under
# `dbus-run-session xvfb-run -a` with openbox as window manager (xprop waits for it). Without
# Docker, run them the same way here, e.g.
# `dbus-run-session xvfb-run -a cargo test -p watane-app --features driver -- --include-ignored`.
# gh: the factory's scripts call it; the Debian and Ubuntu archives have a `gh` package.
apt_sudo=
if [ "$(id -u)" -ne 0 ]; then
  if command -v sudo >/dev/null 2>&1 && sudo -n true >/dev/null 2>&1; then apt_sudo="sudo -n"; else apt_sudo=none; fi
fi
if [ "$apt_sudo" != none ] && command -v apt-get >/dev/null 2>&1; then
  missing=
  for p in pkg-config libwebkit2gtk-4.1-dev libgtk-3-dev xvfb xauth dbus-x11 openbox x11-utils lsof; do
    dpkg-query -W -f='${Status}' "$p" 2>/dev/null | grep -q 'ok installed' || missing="$missing $p"
  done
  command -v gh >/dev/null 2>&1 || missing="$missing gh"
  if [ -n "$missing" ]; then
    # shellcheck disable=SC2086 # split on purpose: the sudo prefix and the package list
    $apt_sudo env DEBIAN_FRONTEND=noninteractive apt-get update -qq >/dev/null 2>&1
    # shellcheck disable=SC2086
    $apt_sudo env DEBIAN_FRONTEND=noninteractive apt-get install -y -qq --no-install-recommends $missing >/dev/null 2>&1 ||
      for p in $missing; do # one name the archive lacks (gh on an old release) must not block the rest
        $apt_sudo env DEBIAN_FRONTEND=noninteractive apt-get install -y -qq --no-install-recommends "$p" >/dev/null 2>&1
      done
  fi
fi

# cargo-mutants at the version .github/workflows/mutants.yml pins: cargo binstall when present, else
# the release's prebuilt x86_64 Linux tarball, else a slow source build.
mutants_version=27.1.0
bin="${CARGO_HOME:-$HOME/.cargo}/bin"
if ! command -v cargo-mutants >/dev/null 2>&1 && [ ! -x "$bin/cargo-mutants" ]; then
  if command -v cargo-binstall >/dev/null 2>&1; then
    cargo binstall -y --quiet --locked "cargo-mutants@$mutants_version" >/dev/null 2>&1
  fi
  if [ ! -x "$bin/cargo-mutants" ] && [ "$(uname -m)" = x86_64 ] && command -v curl >/dev/null 2>&1; then
    if tmp=$(mktemp -d 2>/dev/null); then
      curl -fsSL "https://github.com/sourcefrog/cargo-mutants/releases/download/v$mutants_version/cargo-mutants-x86_64-unknown-linux-gnu.tar.gz" 2>/dev/null |
        tar xzf - -C "$tmp" >/dev/null 2>&1
      found=$(find "$tmp" -type f -name cargo-mutants 2>/dev/null | head -n 1)
      [ -n "$found" ] && mkdir -p "$bin" && install -m 755 "$found" "$bin/cargo-mutants" >/dev/null 2>&1
      rm -rf "$tmp"
    fi
  fi
  [ -x "$bin/cargo-mutants" ] || cargo install -q --locked --version "$mutants_version" cargo-mutants >/dev/null 2>&1
fi
exit 0
