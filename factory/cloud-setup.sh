#!/bin/sh
# SessionStart hook (.claude/settings.json): readies an Anthropic cloud container for watane's tests.
# Does nothing on the Mac.
[ "${CLAUDE_CODE_REMOTE:-}" = true ] || exit 0
cd "${CLAUDE_PROJECT_DIR:-.}" || exit 0
rustup target add wasm32-wasip2 >/dev/null 2>&1
[ -d dev/allowlist/vendor ] || dev/scripts/vendor.sh >/dev/null 2>&1
exit 0
