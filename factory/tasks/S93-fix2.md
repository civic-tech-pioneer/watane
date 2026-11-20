Subsystem: dev
Effort: low
Depends on: none
Slice: S93 — dev's last misleading lines (adversary RISKs, settled by the foreman as cheap fixes).

## Goal
dev/src/builder.rs `sibling_paths` (added by S93-2) names only real sibling copies:
1. A found path whose normalized form is not under `<cache>/build/` (e.g.
   `<cache>/build/hello/../../../etc/passwd`) is not reported.
2. A path from a mod's compiler text holding a control character is not reported (the scan stops a
   path at any control character, like at whitespace), so no raw ESC reaches the
   `build.reads_outside` message or fix.
3. The copy and the build folder are compared in the same form: if the cache path holds a symlink
   (macOS `/var` -> `/private/var`), a missing file inside the mod's own copy is never a sibling.
   Compare after canonicalizing `build` and `copy` once (both exist), and normalize found paths
   lexically against those.

## Acceptance
Unit tests in builder.rs's test module (literals):
- AC1 `sibling_paths("error at /c/build/hello/../../../etc/passwd:1:2", /c/build, /c/build/me)`
  returns nothing.
- AC2 text `/c/build/a\x1b[2J` returns nothing containing `\x1b` (returns `/c/build/a` or nothing;
  pick one, assert it).
- AC3 a temp dir with a symlinked parent: a message naming `<link>/build/me/logic/missing.rs`
  where `copy` is `<real>/build/me` returns nothing.
- AC4 the existing `s93_ac5_*` tests pass unchanged.

## Spec
dev/TECHNICAL 6.4 step 6 (the stale-sibling sentence): add "under the build folder" if absent.

## Interface
None.

## Files
dev/src/builder.rs, dev/TECHNICAL.md 6.4 only.

## Changed tests
None.

## Out of scope
Escaping control characters in dev's text output in general (older than S93; a spec question).
