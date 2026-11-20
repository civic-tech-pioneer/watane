#!/bin/bash
set -euo pipefail
# Resolve relative to this file, so callers can run from any directory.
repo=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
image=watane-app-linux-test
# Stage F uses native arm64 Linux on Apple Silicon, not x86_64 emulation.
docker build --platform linux/arm64 -f "$repo/app/linux.Dockerfile" -t "$image" "$repo/app"
# glibc does not resolve *.localhost as macOS does; the dist tests serve on site.localhost.
docker run --rm --init --platform linux/arm64 --shm-size=1g --add-host site.localhost:127.0.0.1 \
  --mount "type=bind,source=$repo,target=/repo" \
  --mount type=volume,source=watane-app-linux-registry,target=/usr/local/cargo/registry \
  --mount type=volume,source=watane-app-linux-target,target=/linux-target \
  --env NO_AT_BRIDGE=1 --env CARGO_TARGET_DIR=/linux-target --workdir /repo "$image" bash -euc '
    for library in glib-2.0 libsoup-3.0 webkit2gtk-4.1; do
      version=$(pkg-config --modversion "$library")
      printf "%s %s\n" "$library" "$version"
    done
    # One run: the driver build runs the default tests too. Its lone non-driver test
    # (ac1_default_dist_refuses_listed_local) is plain logic that check.sh runs on the Mac.
    # libtest cannot filter by ignore reason. These are every #[ignore = "front: ..."]
    # test in gui_shell/gui_isolation; linux_runner verifies this list stays complete.
    dbus-run-session xvfb-run -a bash -se -- "$@" <<\DRIVER
    openbox >/tmp/openbox.log 2>&1 &
    wm=$!
    deadline=$((SECONDS + 30))
    while :; do
      case $(xprop -root _NET_SUPPORTING_WM_CHECK) in
        *"window id # 0x"*) break ;;
      esac
      if ! kill -0 "$wm" || test "$SECONDS" -ge "$deadline"; then
        echo "Openbox failed to become ready" >&2
        cat /tmp/openbox.log >&2
        exit 1
      fi
      sleep 0.1
    done
    cargo test -p watane-app "$@" --features driver -- --include-ignored \
      --skip ac4_approve_delay_and_deny_focus \
      --skip ac3_reopening_an_open_mod_focuses_it \
      --skip ac3_consent_takes_focus \
      --skip ac1_consent_stays_above_hostile_and_takes_focus
DRIVER
  ' linux-test "$@"
