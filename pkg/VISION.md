# pkg: vision

## Why

Every other subsystem handles mods. If two of them read a mod folder, a
package or a release in two ways, a check in one place is a hole in the
other. `pkg` is the one place that says what a mod is, what it may ask for,
and what bytes stand for it. It is the base of `host`, `dev` and `dist`.

## Goals served

| Goal | How `pkg` serves it |
|---|---|
| V5 Safe by default | The grants come only from the WIT world. A built logic part cannot import more than its world says. Unpacking cannot write outside its folder. |
| V3 Cheap making | Every refusal names the rule, the file and the fix, so a coding agent can act on it alone. |
| V2 Stigmergy | One canonical package and one release body, so a trace means the same bytes for everyone. |
| V4 No center | The package id is a hash of the bytes. A release names it, so any untrusted host can carry the package. |
| V1 Own environment | A mod stays a plain folder that a person can read and change. |

## Boundaries: `pkg` never

- runs, links or compiles code, or starts a process;
- signs, verifies a signature, or knows a channel (`log`, `dist`);
- reads the network, a home, or an install record (`host`, `dist`);
- keeps a second list of grants beside the world;
- decides trust, or what a participant approves;
- depends on another watane subsystem.

## Success

- `host`, `dev` and `dist` read mods, packages and releases only through
  `pkg`, and no rule of section 3 of `spec/TECHNICAL.md` is coded twice.
- Equal folders give equal package bytes on macOS and Linux.
- A hostile package cannot write outside its folder, exhaust memory, or
  smuggle an import past the grants.
- J1, J2 and J3 pass with no change to the formats of this subsystem.
