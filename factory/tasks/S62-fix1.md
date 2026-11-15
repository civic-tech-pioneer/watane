Subsystem: net
Effort: low
Depends on: S62-1
Slice: S62 — Alice pairs her server with a code, and only her own machines are served (net N2)

## Goal

A machine that learns from `hello` that a trusted node holds a newer device list fetches it
(net TECHNICAL 5.2: "a node that sees a higher one asks for it"), so a list the anchor could not
push (it does not know the machine's address before N4) still arrives when the machine reaches
the anchor or another own machine. A pairing code pasted with surrounding whitespace still parses.

## Acceptance

AC1 The red test `net/tests/regress_s62.rs::regress_s62_hello_with_newer_devices_seq_pulls_the_list`
passes: after `hello` returns a `devices_seq` above the stored seq, the caller asks that node
`devices(own seq)` and stores the answer under the rules of 5.5 (stored anchor signed it, higher
seq), before `hello` returns or within the test's 5 s. A failed pull changes nothing and does not
fail `hello`.
AC2 `PairCode::from_str` trims leading and trailing ASCII whitespace (a copied line with a newline
parses); whitespace inside the code is still `BadCode`. A unit or `tests/code.rs` test shows both.
AC3 Nothing else changes; the full net suite, clippy -D warnings and fmt stay clean.

## Spec

net/TECHNICAL.md 4.1 (add one sentence: surrounding whitespace is trimmed), 5.2, 5.5; and fix the
N2 note in section 9 that says N2 sends no list on `hello`: a caller pulls on a higher
`devices_seq` (N3 still does the syncing of a machine that was offline on connect).

## Interface

None changes.

## Files

net/src/client.rs, net/src/lib.rs, net/src/pairing.rs, net/src/ticket.rs, net/tests/code.rs,
net/TECHNICAL.md (4.1, section 9 note). Do not edit regress_s62.rs except to fix a mistake in it.

## Changed tests

none (searched: tests/code.rs asserts refusals of malformed codes, none of surrounding whitespace).

## Out of scope

Learning addresses from incoming connections (N4). `CodeSpent` distinctions (deliberately none).

## Cloud rules (binding)

127.0.0.1 only, no internet, no /proc. `CARGO_PROFILE_DEV_DEBUG=0`, shared target, clean /tmp/.tmp*.
`cargo test -p watane-net --no-fail-fast`.
