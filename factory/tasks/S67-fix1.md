Subsystem: net
Effort: low
Depends on: S67-1
Slice: S67 — Alice's light laptop searches and installs through her server (net N4, part 1)

## Goal

user-sim found that a package the full machine does not serve (it lacks it, or its own copy failed
its check, so it cuts the request) comes back from `need_blob` with iroh's transport words:
"node X is unreachable: package 07070707 was not fetched: io: stream reset by peer: error 1".
NET-F-32 asks for the cause in watane's words. After this task the cause reads, for example,
"package 07070707 is not served by <short key>: it does not have it, or its copy failed its check",
and iroh's error text does not appear in it.

## Acceptance

AC1 `tests/regress_s67.rs` `regress_s67_a_missing_package_has_a_cause_in_watanes_words` passes,
unchanged. Nothing else changes: the variant stays `NetError::Unreachable` naming the last machine
asked (AC6 of S67-1), a hash mismatch keeps its "does not match its hash" wording, and every other
test keeps passing.

AC2 The mapping lives in `failed()` of `net/src/blobs.rs` (it passes any `GetError` other than
`Decode` through as `e.to_string()` today): a request the serving machine cut or refused, or an
empty answer, gets the watane cause above; any other transport failure gets a short watane cause
that names the package's hex8 (e.g. "package <hex8> could not be read from <short>: the connection
failed"), not iroh's text. Same rule for the sync path's event cause, if it uses the same function.
5.9 or the N4 notes of net/TECHNICAL.md 9 get one line on the wording.

AC3 clippy `-D warnings`, fmt, and net's default `cargo test --no-fail-fast` pass.

## Spec

net/FUNCTIONAL.md NET-F-32; net/TECHNICAL.md 3.2, 5.8, 5.9.

## Interface

None changes. `NetError`'s variants and their display stay as they are.

## Files

net/src/blobs.rs (and net/src/light.rs only if the cause is built there), net/TECHNICAL.md.

## Changed tests

none (searched tests/light.rs and tests/sync.rs for "not fetched" and "stream reset": no
assertion names iroh's wording).

## Out of scope

Telling "does not have it" apart from "its copy is bad" (needs a wire answer; later).
