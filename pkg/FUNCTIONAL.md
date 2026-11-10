# pkg: functional specification

What `pkg` does, free of implementation. `TECHNICAL.md` says how. Ids in
`Traces:` refer to `spec/FUNCTIONAL.md`. "pkg" below is the subsystem.

## Step 1: Make

### Read a mod folder

- **PKG-F-1** When a caller gives a mod folder, pkg shall give the slug, the version, the name, the summary, the parts (page, logic part), the interfaces that the mod offers, the events that it sends, the interfaces that it uses, its providers and its grants.
  Traces: F-MAKE-2, F-RUN-1, F-INST-1, J1.
- **PKG-F-2** If a mod folder breaks a rule, then pkg shall refuse it and name each broken rule with a stable rule code, the file and a message that says what to change.
  Traces: F-MAKE-2, F-MAKE-5, F-QUAL-3, J1.
- **PKG-F-3** pkg shall accept a mod only if it has at least one part, at most one page and at most one logic part.
  Traces: F-RUN-1.
- **PKG-F-4** pkg shall take the identity of the mod (slug and version) from its WIT world, so that no other file repeats it.
  Traces: F-MAKE-1, F-PUB-2.

### Grants

- **PKG-F-5** pkg shall derive the grants of a mod only from its WIT world, its provider list and its list of network hosts. There is no other list of grants.
  Traces: F-RUN-2, F-RUN-7, F-INST-1, F-INST-2.
- **PKG-F-6** pkg shall give each grant one of the four kinds of F-RUN-7: private storage, network access to one named host, a call to a named interface of a named provider, a subscription to the events of a named interface of a named provider.
  Traces: F-RUN-7, F-APP-5.
- **PKG-F-7** pkg shall name the provider of each call and subscription with its slug and its publisher, or as local. A WIT package name alone never names a provider.
  Traces: F-RUN-5, F-RUN-6, F-INST-1.
- **PKG-F-8** If a world imports or exports an interface that watane does not offer, then pkg shall refuse the mod and name the interface.
  Traces: F-RUN-2, F-QUAL-3.

### Built logic part

- **PKG-F-9** When a logic part is built, pkg shall check that it imports only the interfaces of its world and the base WASI interfaces, and that it exports exactly the exports of its world.
  Traces: F-RUN-2, F-RUN-5, F-MAKE-2.

### Package

- **PKG-F-10** pkg shall pack a built mod folder into one package. Equal content shall give equal bytes on every device.
  Traces: F-PUB-2, F-MAKE-4.
- **PKG-F-11** pkg shall give the package id, which is the BLAKE3 hash of the package bytes.
  Traces: F-PUB-2, F-INST-6.
- **PKG-F-12** pkg shall put into a package only the files of the mod, and the source only when the caller asks for it. It shall never include hidden files, build output, or a file reached through a link.
  Traces: F-PUB-4, F-RUN-8, V5.
- **PKG-F-13** If a package breaks a format rule or a limit, then pkg shall refuse it before it writes a file, and name the rule.
  Traces: F-RUN-2, F-INST-6, F-QUAL-3.
- **PKG-F-14** When pkg unpacks a package, it shall write only inside a new folder, and give the checked mod or no folder at all.
  Traces: F-RUN-2, F-INST-2, V5.

### Shared text forms

- **PKG-F-15** pkg shall define one text form for a slug, a version, an interface name, a network host, a publisher key, a package id and a release reference, and every subsystem shall use these forms.
  Traces: F-INST-1, F-APP-3, F-APP-5, F-MAKE-5.

## Step 2: Run together

- **PKG-F-16** For each provider of a mod, pkg shall give the slug, the publisher or local, and the lowest version that the mod accepts.
  Traces: F-INST-1, F-INST-4, J2.
- **PKG-F-17** pkg shall give one version rule: when an offered version satisfies a wanted version.
  Traces: F-INST-4, F-RUN-5, J2.
- **PKG-F-18** When a maker adds a provider to a mod folder, pkg shall copy the interface of the provider into the folder and record the provider with its publisher or as local.
  Traces: F-MAKE-3, F-BUILD-2, J2.
- **PKG-F-19** pkg shall give each grant a stable text and data form, so that the host can record it and the app can show it.
  Traces: F-INST-1, F-INST-5, F-APP-5.

## Step 3: Publish

- **PKG-F-20** When a caller gives a mod and its package, pkg shall make the release body: slug, version, name, summary, package id, package size, whether the source is included, the offered interfaces, the events, the grants and the origin.
  Traces: F-PUB-2, F-PUB-4.
- **PKG-F-21** If a mod depends on a local provider, then pkg shall refuse to make its release body and name the provider.
  Traces: F-PUB-2, F-INST-6.
- **PKG-F-22** pkg shall encode a release body in one canonical form, and refuse to decode any other form.
  Traces: F-INST-6, F-PUB-7.
- **PKG-F-23** When a caller gives a release body, a package and the mod of that package, pkg shall confirm that they agree, or name the first field that differs.
  Traces: F-INST-6, F-QUAL-3, J3.

## Later steps (only enough to not block them)

- **PKG-F-24** (step 4) pkg shall tell whether two grants give the same authority, apart from the version of the interface.
  Traces: F-INST-7, J4.
- **PKG-F-25** (step 5) pkg shall read and keep the origin of a derived mod, and carry it in the release body.
  Traces: F-BUILD-1, F-BUILD-4, J5.

## Acceptance scenarios

Each scenario is one test. The expected values come from this file or from
`TECHNICAL.md`, never from the code under test.

| # | Given | When | Then | Checks |
|---|---|---|---|---|
| A1 | the folder `reading-list` of `TECHNICAL.md` section 9 | it is read | slug `reading-list`, version `0.1.0`, a page and a logic part, offers `mods:reading-list/books@0.1.0`, events `mods:reading-list/events@0.1.0`, grants exactly `[storage]` | PKG-F-1, PKG-F-5 |
| A2 | the folder `recommendations` of section 9, with a local provider | it is read | grants exactly `[storage, call mods:reading-list/books@0.1.0 from local/reading-list, subscribe mods:reading-list/events@0.1.0 from local/reading-list]`; one requirement: `local/reading-list` at `0.1.0` | PKG-F-5, PKG-F-7, PKG-F-16 |
| A3 | a world that imports `wasi:http/outgoing-handler@0.2.0` and `mod.toml` with hosts `a.example` and `b.example` | it is read | grants include `http a.example` and `http b.example` | PKG-F-6 |
| A4 | the same world with no `[http]` hosts | it is read | refused with rule `http`, file `mod.toml` | PKG-F-2 |
| A5 | a world that imports `wasi:keyvalue/store@0.2.0` | it is read | refused with rule `import`, and the message names `wasi:keyvalue/store@0.2.0` | PKG-F-8 |
| A6 | a world that imports `mods:tags/api@0.1.0` and `deps.json` without `mods:tags` | it is read | refused with rule `provider`, file `deps.json` | PKG-F-7 |
| A7 | a folder with no `ui/index.html` and a world with no export | it is read | refused with rule `parts` | PKG-F-3 |
| A8 | `mod.toml` with a field `slug` | it is read | refused with rule `mod-toml`; the message says that the slug comes from the package line of `wit/world.wit` | PKG-F-2, PKG-F-4 |
| A9 | a component made from a world with one more import than `wit/world.wit` | the component is checked | refused with rule `component`, naming the extra import | PKG-F-9 |
| A10 | a component that also imports `wasi:cli/environment@0.2.3` | the component is checked | accepted | PKG-F-9 |
| A11 | the folder `hello` of `TECHNICAL.md` section 6.5 | it is packed without source | the bytes equal the hex of section 6.5 and the id is `0866e9df433265eaf0d959c589d82f1c2020d5b07faf7a530b012ec9f62ff87b` | PKG-F-10, PKG-F-11 |
| A12 | two copies of one folder, made in a different order, with different times and modes | both are packed | the bytes are equal | PKG-F-10 |
| A13 | a folder with `.git/`, `logic/target/`, `.DS_Store`, `notes.md` and `logic/src/lib.rs` | it is packed without source, then with source | without: none of these; with: only `logic/src/lib.rs` is added | PKG-F-12 |
| A14 | a file `ui/key` that is a link to a file outside the folder | it is packed | refused; the error names `ui/key` | PKG-F-12 |
| A15 | a package with a path `ui/../../x`, `/x`, `ui//x`, `ui/.x`, or both `ui/A.js` and `ui/a.js` | it is decoded | refused with the path named, and nothing is written | PKG-F-13 |
| A16 | a package of 64 MiB plus one byte, or a record whose size passes the end of the input | it is decoded | refused with no allocation larger than the input | PKG-F-13 |
| A17 | a valid package and a folder path that exists | it is unpacked | refused; the existing folder is unchanged | PKG-F-14 |
| A18 | a valid package whose `mod.toml` breaks a rule | it is unpacked | refused with rule `mod-toml`; the target folder does not exist afterwards | PKG-F-14 |
| A19 | versions | `compatible` is asked | the table of `TECHNICAL.md` section 5.2 holds | PKG-F-17 |
| A20 | the folders `reading-list` and an empty consumer | the provider is added as local | `wit/deps/reading-list/` holds the provider's top-level `.wit` files, `wit/deps/watane-host/` is copied, and `deps.json` is exactly `{\n  "mods:reading-list": null\n}\n` | PKG-F-18 |
| A21 | the mod and package `hello` | the release body is made and encoded | the bytes equal the hex of `TECHNICAL.md` section 7.3 | PKG-F-20, PKG-F-22 |
| A22 | the mod `recommendations` with a local provider | the release body is made | refused, naming `reading-list` | PKG-F-21 |
| A23 | the release hex of section 7.3 with one change: a non-shortest integer, an indefinite array, an extra item, or trailing bytes | it is decoded | refused | PKG-F-22 |
| A24 | a release body whose `summary` differs from the package's `mod.toml` | it is checked against the package | refused with field `summary` | PKG-F-23 |
| A25 | the publisher key of 32 bytes `0x01` | it is written as text | `yryonyebyryonyebyryonyebyryonyebyryonyebyryonyebyryo` | PKG-F-15 |
