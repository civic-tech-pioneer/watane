# log: vision

## Why

With no operator, a person must be able to check who said what, and in
which order, from bytes that any untrusted web host can serve. `log` gives
each participant an identity, and a signed, hash-chained channel of
statements. It gives every reader the one strict rule to verify them.

## Goals served (`spec/VISION.md`)

| Goal | How `log` serves it |
|---|---|
| V4 No center | An identity is a key that the device creates. A statement verifies with its bytes and the key alone: no account, no server, no trusted host. |
| V5 Safe by default | Each statement is signed and chained. A changed byte, a forged statement or a rewritten history fails, with the cause named. |
| V6 Chosen trust | A person trusts a key, not an authority. Two contradicting statements of one key mark that channel compromised, for every reader in the same way. |
| V2 Stigmergy | Every published trace has an author that a reader can check. |
| V7 Path to explicit collective rationality | The envelope carries any `kind` and opaque body, so later statements need no new envelope. |

## Boundaries: what `log` must never become

- **Never a network client.** `log` reads and writes local files only.
  Moving bytes is `dist`.
- **Never a reader of meaning.** The body is opaque bytes. `pkg` and `dist`
  own the bodies; `dist` decides which kinds exist.
- **Never a general signing oracle.** The only way to sign is to append a
  statement to the own channel. The secret key never leaves `log`, except
  encrypted in a backup.
- **Never the place that asks.** `log` does not ask for confirmation. Its
  caller (`dist`) asks the app before each append (F-AGENT-4, W13).
- **Never a trust policy.** `log` says "valid", "gap" or "compromised".
  Whom to trust is the participant's choice (W11).
- **Never more than one identity per directory**, and no key server or
  escrow.

## Success

- `dist` and `host` use `log` for every signature and every check; no other
  code in watane parses or verifies an entry.
- A third party can write a verifier from `TECHNICAL.md` alone, byte for
  byte, and agree with `log` on every test vector.
- J3 and J4 pass on the real programs: Bob verifies Alice's release, and a
  second statement at a used position marks Alice's channel compromised.
- The entry format and the backup format do not change after step 3.
