# dev: vision

## Why

A coding agent can write code, but it does not know the watane constraints,
and it must not get authority. `dev` is the program `watane-dev`: one
command line with which a maker and their coding agent create, check, build,
test and preview a mod, and then hand it to the participant for approval.
Its commands are its product. Each command gives a result that an agent can
read and act on without a person.

## Goals served

| Goal | How `dev` serves it |
|---|---|
| V3 Cheap making | A template that passes its tests at once, results that name the fix, and a preview in the same session. |
| V5 Safe by default | A build that runs no code of the mod on the device, outside a sandbox. A mod is installed or published only through the home host, which asks the participant. |
| V2 Stigmergy | Contract tests with fakes let a maker build on the interface of another mod. Step 5 adds `new --from` and `add <release>`. |
| V4 No center | No account, no key, no server. The build is offline. |

## Boundaries: `dev` must never

- hold a key, sign, or approve anything (F-MAKE-6, F-AGENT-4);
- run a mod for use, or install a mod in a home other than its own preview
  home without the home host;
- open a network connection, except HTTP to a loopback address of a host;
- install a toolchain or fetch a crate;
- run code of the mod, or of its tests, natively on the device;
- write the code of a mod: the coding agent does that.

## Success

1. A coding agent that has never seen watane runs `watane-dev new`, reads
   the `AGENTS.md` of the new folder, and gets a mod that passes
   `watane-dev test` with no help from a person.
2. Each failure names a code, a file and line where it has one, and a fix.
   The agent fixes the cause in the next try.
3. `watane-dev serve` gives a page address and a toolset token in seconds
   after the first build. A second `serve` shows the change and keeps the
   preview data.
4. `install` and `publish` return when the participant answers in the app,
   with the answer.
5. J1 passes on the real programs.
