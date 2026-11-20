Subsystem: dev
Effort: medium
Depends on: none
Slice: S93 — dev's last misleading lines (user-sim REJECTED: fixes that are not commands).

## Goal
The red tests `build_on::regress_s93_no_source_fix_runs_as_written` (4e0388b),
`publish::regress_s93_declined_and_version_exists_fixes_end_in_a_command` and
`install::fake::regress_s93_install_declined_next_is_a_command` (d8b29b9) pass unedited. dev/TECHNICAL
2.2 pinned these texts against 2.4's own rule; change 2.2 first, then the code:
1. `usage.no_source` (new --from a release without source): the fix must run as written. It
   becomes `Build on a release that includes its source, or make a new mod that depends on this
   one: watane-dev new <slug> --dir <dest> && watane-dev add --release <ref> --dir <dest>[ --home
   <h>]` (the slug and destination `new --from` would have used, quoted), or another form the red
   test accepts — read it first and match it.
2. `host.declined` (install, publish): no `<seconds>` placeholder. Fix: `The person declined in
   the app. Ask them, then run: <rerun as given>`; `next` the rerun.
3. `publish.version_exists`: fix ends `, then run: <rerun>` (the step it names stays). The
   provider fix of api.rs ~:470 that ends `then run install again` ends in its rerun the same way.
4. `new <slug>` without `--dir`: its `next` (`... then run: watane-dev serve --json`) carries
   ` --dir <the new folder, quoted>` so it runs from where `new` ran (user-sim "felt wrong").
Leave `search`'s no-result template (2.4 pins it) and the other "felt wrong" items alone.

## Acceptance
- AC1 the three red tests pass unedited (run the slow ones with `--ignored` if marked).
- AC2 a grep of dev/src for `run install again`, `run publish again`, `<seconds>` finds nothing in
  a fix or next.
- AC3 `new hello --json` in an empty folder: `next` ends `--dir <abs>/hello` (or the relative
  `hello`, quoted; pick one, assert the literal).
- AC4 every other dev test passes; each changed existing test listed in your last message.

## Spec
dev/TECHNICAL.md 2.2 (rows `usage.no_source`, `host.declined`, `publish.version_exists`), 2.4
(`new`'s next).

## Interface
None.

## Files
dev/src/{cli,api,build_on,share}.rs as needed, dev/tests/it/{build_on,publish,install,…}.rs for
changed existing assertions only, dev/TECHNICAL.md 2.2/2.4. Not dev/src/builder.rs (S93-fix6).

## Changed tests
Search dev/src and dev/tests/it for the old texts (`run install again`, `run publish again`,
`<seconds>`, `depend on this one with`, `then run: watane-dev serve --json`) and update each.

## Out of scope
builder.rs, pkg.
