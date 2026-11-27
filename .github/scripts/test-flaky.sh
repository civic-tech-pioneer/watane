#!/usr/bin/env bash
# .github/scripts/test-flaky.sh <features> [<libtest args>...]: CI's test step with flake detection
# (factory/README.md, "Flaky tests"). Runs the workspace tests; when some fail, reruns only those,
# twice, on the same commit. A test that passes on a rerun is flaky: it does not fail the job, and
# gets an open `flaky` issue (one per test; a later run adds a comment). A test that fails every
# time fails the job. Plain files, no arrays: macOS runs bash 3.2.
set -u

features=$1
shift
dir=$(mktemp -d)

failed() { sed -n 's/^test \(.*\) \.\.\. FAILED\r\{0,1\}$/\1/p' "$1" | sort -u; }

cargo test --workspace --locked --features "$features" --no-fail-fast -- "$@" 2>&1 | tee "$dir/log"
status=${PIPESTATUS[0]}
[ "$status" -eq 0 ] && exit 0

failed "$dir/log" >"$dir/first"
if [ ! -s "$dir/first" ]; then
  echo "::error::cargo test failed ($status) without a failing test (a build error?): no rerun"
  exit "$status"
fi

cp "$dir/first" "$dir/still"
for attempt in 1 2; do
  echo "::group::rerun $attempt: $(wc -l <"$dir/still" | tr -d ' ') failed test(s)"
  # shellcheck disable=SC2046 # one argument per test name; names have no spaces
  cargo test --workspace --locked --features "$features" --no-fail-fast -- --exact $(cat "$dir/still") 2>&1 | tee "$dir/log"
  echo "::endgroup::"
  failed "$dir/log" >"$dir/still"
  [ -s "$dir/still" ] || break
done

comm -23 "$dir/first" "$dir/still" >"$dir/flaky"

run_url="${GITHUB_SERVER_URL:-https://github.com}/${GITHUB_REPOSITORY:-}/actions/runs/${GITHUB_RUN_ID:-}"
while read -r t; do
  echo "::warning::flaky: $t failed, then passed on a rerun"
  [ -n "${GH_TOKEN:-}" ] || continue
  title="flaky: $t"
  body="\`$t\` failed and then passed on a rerun of the same commit (\`${GITHUB_SHA:-?}\`, ${RUNNER_OS:-?}, ref \`${GITHUB_REF:-?}\`): $run_url"
  # Filing the issue never fails the job.
  n=$(gh issue list --label flaky --state open --search "\"$title\" in:title" --json number,title \
        --jq ".[] | select(.title == \"$title\") | .number" 2>/dev/null | head -1)
  if [ -n "$n" ]; then
    gh issue comment "$n" --body "$body" >/dev/null || echo "::warning::could not comment on #$n"
  else
    gh label create flaky --color B60205 --description "Failed, then passed on the same commit (CI)" >/dev/null 2>&1
    gh label create upkeep --color 0E8A16 --description "Small work for an upkeep slice" >/dev/null 2>&1
    gh issue create --title "$title" --label flaky --label upkeep --body "$body

Find the cause. A guard test is fixed at its cause; a polish one may get \`#[ignore = \"flaky: <cause>, accepted <date>\"]\` after one diagnosis (factory/README.md, \"Time and value\")." >/dev/null \
      || echo "::warning::could not open an issue for $t"
  fi
done <"$dir/flaky"

if [ -s "$dir/still" ]; then
  sed 's/^/::error::failed on every run: /' "$dir/still"
  exit 1
fi
exit 0
