# Vision: watane

watane is an empty host that people fill with their own mods. Makers write
mods with a coding agent of their choice and the watane maker tools. Each
person publishes mods to a personal channel, and other people find, use and
extend them. No operator sits in the middle.

## Why

People do not act well together on what they want and know. Their desires,
beliefs and intentions are scattered. Coordination is expensive, and the
platforms that make it cheaper also own it. We want higher levels of
collective intelligence: groups that notice what they need, know what is
true, and act on it together.

Collective rationality has two routes:
- **Explicit:** people state desires, beliefs, plans and evidence, and
  reason about them together.
- **Implicit (stigmergy):** people leave traces, such as software and data,
  and others pick up and extend those traces. A mod expresses what its
  maker wants and believes, without a debate.

watane serves the implicit route. It is a narrow substrate: a runtime that
connects mods through typed interfaces, safe installation of other people's
work, and a way to find that work. The explicit route arrives later as mods
on this substrate.

Two changes make this possible now. Agents bring the cost of software close
to zero, so anyone can use software to express an intention. Isolated
runtimes and signed publication make it possible to share and run code
without an operator and without blind trust.

## Goals

- **V1 Own environment.** Each person shapes watane into the tool they
  need. It starts empty and contains only what the person adds.
- **V2 Stigmergy.** Every published mod is a trace that other people can
  find, run, compose with and derive from. Work builds on work.
- **V3 Cheap making.** An agent turns a description of a need into a
  working mod. A person does not need to program to make or change a mod.
- **V4 No center.** Nobody needs a watane operator or a watane account to
  publish, find, install or trust a mod. Publication can use any untrusted
  static host that the maker chooses.
- **V5 Safe by default.** A person can run a stranger's mod without risk to
  their data or their other mods. Each authority is explicit, minimal and
  visible, and the person can withdraw it.
- **V6 Chosen trust.** Trust comes from the people that each person
  chooses. No global authority decides what is good.
- **V7 Path to explicit collective rationality.** watane keeps the way open
  for mods that exchange signed statements between people. watane itself
  does not carry these statements.

## Order of work

Each step must work before the next one starts. A question that only a
later step needs waits until that step is next.

1. **Make.** A coding agent makes a mod under the watane constraints.
2. **Run together.** Mods run in one host, with their dependencies wired.
3. **Publish.** A maker publishes a mod package.
4. **Find.** A person finds the packages of others.
5. **Build on.** A maker builds on an existing package.
6. **Factory (later).** A set of mods that steers coding agents and changes
   itself. It runs in the same host and is not part of the substrate.

After the first version:

- **Own machines (step 8).** A person's machines keep each other current
  directly, with no static host between them and no endpoint that someone
  else runs. Friends, strangers and public peers come later (`PEERS.md`
  10). Step 7 is the build plan's own "real world" step
  (`factory/PLAN.md`).

## Success of the first version

The first version is steps 1 to 5. It succeeds when these things happen
with no server that a participant must trust:

1. A person asks their coding agent for a mod and tries it in a preview in
   the same session.
2. The person installs it beside other mods and uses them together.
3. The person publishes the mod. A second person installs it from a link
   and uses it.
4. A third person finds it by describing a need, and installs it.
5. That person derives a new mod from it, or builds on its interface, and
   publishes the result. The derivation is visible.

## Later, not in the first version

- Audits and trust policies (V6). The first version gives signatures,
  isolation and consent.
- Direct transfer between devices, without a static host: between own
  machines in step 8, between friends and strangers after it.
- The factory (step 6), and the statements of V7.
- Private channels, and live sessions of several people.

## Non-goals

- watane is not an app store. Nobody curates, ranks or removes mods for
  everyone.
- watane is not an operating system. It is one host, a desktop app around
  it, and maker tools.
- watane has no token, payment or allocation mechanism.
