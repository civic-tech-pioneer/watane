# watane (working name)

An empty host that people fill with their own mods.

`watane/` is one git repository. This folder, `spec/`, holds the
specification of the whole. Each subsystem is a folder beside it, with its
own `VISION.md`, `FUNCTIONAL.md`, `TECHNICAL.md` and `AGENTS.md`.

## Read in this order

1. [VISION.md](VISION.md): why, the goals, and the order of work.
2. [FUNCTIONAL.md](FUNCTIONAL.md): what watane does, and one journey for each step.
3. [TECHNICAL.md](TECHNICAL.md): the subsystems, what flows between them, and the security model.
4. `../<subsystem>/`: the same three documents for one subsystem, and `AGENTS.md` for the agent that builds it.

| Subsystem | Reason to exist |
|---|---|
| [pkg](../pkg/VISION.md) | Defines what a mod and a package are. |
| [host](../host/VISION.md) | Runs the installed mods together and guards every authority. |
| [dev](../dev/VISION.md) | Lets a maker and a coding agent make a mod. |
| [app](../app/VISION.md) | Gives the participant a window, and the one place for every approval. |
| [log](../log/VISION.md) | Makes statements that others can verify. |
| [dist](../dist/VISION.md) | Moves releases between people. |

## Rules

1. **The chain decides.** Vision, then functional, then technical, then the
   documents of a subsystem, then code. When two disagree, fix one of them
   at once.
2. **One fact in one place.** `TECHNICAL.md` says what flows between
   subsystems. A signature is only in the `TECHNICAL.md` of the subsystem that owns it.
   Another subsystem cites it and never redefines it.
3. **Design up to step 3 only.** Steps 4 and 5 get only what keeps them
   open, marked as such, not a guess.
4. **A step is done when its journey passes** on the real programs.
5. **`../colos` is reference.** Code is written new here.
