# Make this page

Edit ui/index.html for the page and ui/app.js for its behavior. Add page assets
in ui/. mod.toml holds the name and summary; wit/world.wit holds the slug and
version. Keep this guide and CLAUDE.md for coding agents.

Work autonomously; do not ask questions. Edit ui/, run `watane-dev check --json`,
then `watane-dev serve --json` and give result.pages[0].url to the person.
Scripts must be files in ui/: no inline scripts and no CDN.

With --json each command prints one envelope: schema (1), command, ok, dir,
result, errors, warnings, next. Read errors[].fix and correct the cause.
Exit statuses: 0 success, 1 mod error, 2 usage, 3 environment, 4 host refusal,
5 internal error. Each error has code, message, file, line, column and fix;
unknown locations are null. Warnings do not change the exit status.
