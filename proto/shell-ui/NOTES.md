# Prototype: a pluggable front end with today's primitives (layer 1)

Throwaway. No change to `host`, `app`, `pkg` or `dist`. It feeds the design
of a fan-out "collect" call.

- `shell/`: a page with a menu, a toolbar and a status line. Its registry
  is kept in private state. It exports `registry` (`add`, `list`,
  `notify`, `status`) and `ui` (`start`, `click`). It sends two events:
  `announce` and `commands.invoke(id)`.
- `greeter/`: logic only. It depends on `local/shell`. Its `announce`
  handler calls `registry.add` twice (a menu item and a toolbar button).
  Its `invoke` handler counts greetings and calls `registry.notify`.

Run it:

```sh
cd proto/shell-ui/greeter && ../../../target/debug/watane-dev serve --json
```

Then open `pages[0].url` (the shell page). Stop it with `serve --stop`.

## Result (2026-11-15)

- `watane-dev test`: shell has 8 contract steps and 1 unit test, greeter
  has 9 contract steps and 1 unit test. All pass.
- In one preview host, the shell page shows "Say hello" in the menu and
  "Reset greetings" in the toolbar, both from greeter.
- Clicks round-trip in the browser: "greeter: hello #2", then
  "greeter: greetings reset". There are no console errors.

## What worked

- **No timing problem.** Delivery is synchronous: host TECHNICAL 9.2
  delivers every event before the top-level call returns. So when
  `ui.start` returns, every contributor has already called
  `registry.add`. The page just calls `start`, then `list`.
- **Calling back into the emitter is allowed.** A delivery is a new chain,
  with caller `mod:shell`. greeter's call to `shell/registry` is an
  ordinary call link (greeter → shell). The subscription links (shell →
  greeter) are not part of the cycle check, and host 1423 says
  "Subscription cycles are allowed". Hops: announce is hop 1, and the add
  is a nested call within it. That is well under the limit of 8.
- `watane-dev add ../shell` printed exactly the import and export lines
  the contributor needed.

## Where announce-and-call-back hurt

1. **Clicks cannot be addressed.** `invoke(id)` goes to every
   contributor, and each one filters by id. Ids form one namespace that
   every contributor shares. Any caller of `registry.add` (another mod,
   or the shell's own page) can register an item with greeter's id. The
   probe registered "Free prize" with id `greeter.hello`, and clicking it
   ran greeter's handler. The shell knows the owner (`caller.who()`), but
   it cannot use that: greeter does not know its own local name, so it
   cannot check an owner field.
2. **The registry is derived data kept as state.** `start` clears and
   rebuilds it on every page load: writes, a `changed` message, and a
   second read. The shell gets no signal when a contributor is installed
   or removed. A new contributor appears only after the next `start`. A
   removed contributor's items stay until then, and clicking them does
   nothing.
3. **Failures are silent.** If a contributor's `announce` handler fails,
   the only trace is a log line. The shell cannot tell "contributes
   nothing" from "broken".
4. **Grants are too broad and too many.** One menu item needs three
   grants: call `shell/registry`, subscribe to `announce`, subscribe to
   `commands`. The call grant lets greeter call `add`, `notify` and `list`
   at any time, not only while answering `announce`. The consent words
   ("call shell registry") do not say "adds a menu item".
5. **Every click costs O(N).** Each click delivers to every contributor,
   and so does each announce. `limits.deliveries` is 256 per top-level
   call, which also caps the number of contributors.
6. **Contract tests cannot check the contribution.** A fake answers
   `registry.add`, but a test step cannot assert what was added. greeter's
   real output (its items) is tested only by hand.
7. **Order is local-name order.** That is the delivery order, not
   something the shell or the participant chose.

## What a fan-out collect call would fix

Say the shell calls `collect contributions.items()`, and the host calls
every granted implementer and returns their answers, each tagged with its
mod:

- (1) The click goes back to the mod that answered, using the host's tag,
  not an id string. Hijacking is impossible.
- (2) Answers are return values, not state. Every collect sees the current
  installs, so there is nothing to clear and nothing stale.
- (3) A failure can come back per contributor.
- (4) A contributor needs one grant: the shell calls an interface the
  contributor exports. It needs no call grant into the shell.
- (6) A contract test asserts the returned items directly.

Open questions for the collect design: whether one failing contributor
fails the whole call, the limit on answers, the order, and whether the
caller sees contributors' local names.
