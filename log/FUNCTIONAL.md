# log: functional specification

What `log` does, free of implementation. `TECHNICAL.md` says how. Ids in
`Traces:` refer to `spec/FUNCTIONAL.md`.

## Terms

| Term | Meaning |
|---|---|
| identity | The key pair of the participant. `log` holds at most one for each directory; a replica's holds none (LOG-F-22, step 8). |
| entry | One signed statement: author, position, link to the previous entry, time, kind, body, signature. |
| position | The sequence number of an entry in its channel. The first entry has position 0. |
| channel | The entries of one author, in position order. Each entry links to the entry before it. |
| own channel | The channel of the identity of the directory. |
| head | The stored entry with the highest position of a channel. |
| gap | A given entry that does not directly follow the head, so it cannot be checked yet. |
| fork proof | Two validly signed entries of one author that cannot both be in one chain. |
| compromised | The state of a channel for which `log` holds a fork proof. |
| backup | The secret key, encrypted with a key that comes from a passphrase. |
| caller | The subsystem that uses `log`: `dist` (all functions), `host` (reading entries) and, from step 8, `net` (keys and reading entries). |

## Steps 1 and 2

`log` has no function in steps 1 and 2. A preview has no identity (W5).

## Step 3: Publish

### Identity

- **LOG-F-1** When the caller opens a directory that holds no identity, `log` shall create a new identity on the device, with no network. Traces: F-APP-2, J3.
- **LOG-F-2** `log` shall keep the secret key in a file that only the user can read, and it shall never give the secret key out, except encrypted in a backup. Traces: F-APP-2, F-APP-6.
- **LOG-F-3** If the key file or its directory is open to other users, or the key file is not a plain file of the right length, then `log` shall refuse to open, change nothing, and name the path and the cause. Traces: F-QUAL-3.
- **LOG-F-4** `log` shall give each public key exactly one text form and one short form for display, and shall refuse every other text as a key. Traces: F-INST-1, F-APP-5, F-PUB-6.

### Backup

- **LOG-F-5** When the caller asks for a backup with a passphrase, `log` shall give the backup of the identity, encrypted with a key that it derives from the passphrase. If the passphrase is empty, then `log` shall refuse. Traces: F-APP-6.
- **LOG-F-6** When the caller restores a backup with its passphrase, `log` shall make the identity of the backup the identity of the directory. If the current identity has an entry in its own channel, then `log` shall refuse and keep the current identity. Traces: F-APP-6.
- **LOG-F-7** If a backup is damaged, has an unknown format, or the passphrase is wrong, then `log` shall refuse the restore, name the cause, and change nothing. Traces: F-APP-6, F-QUAL-3.
- **LOG-F-8** After a restore, `log` shall add no entry to the own channel until the caller confirms that `log` holds the newest entry of that channel. Traces: F-APP-6, F-PUB-7.

### Entries

- **LOG-F-9** When the caller appends a kind and a body, `log` shall sign an entry with the identity, at the next position of the own channel, linked to the head, and store it. Traces: F-PUB-1, F-PUB-2, J3.
- **LOG-F-10** `log` shall offer no way to sign other than an append to the own channel. Traces: F-AGENT-4.
- **LOG-F-11** `log` shall encode each entry in exactly one way, so that each entry has one byte form and one id. Traces: F-PUB-2, F-PUB-3.
- **LOG-F-12** When the caller gives the bytes of an entry, `log` shall accept them only if the encoding, the limits and the signature pass one strict rule. Else it shall refuse them and name the first failed check. Traces: F-INST-6, F-PUB-6, F-QUAL-3, J3.
- **LOG-F-13** `log` shall treat the body as opaque bytes and the kind as a short name. It shall not read their meaning. Traces: F-PUB-2.

### Channels

- **LOG-F-14** When the caller gives entries of one author, `log` shall store, in position order, each entry that directly follows the stored head of that channel. Traces: F-PUB-6, J3.
- **LOG-F-15** If a given entry does not directly follow the stored head, then `log` shall keep it out and report the lowest position that it kept out. Traces: F-PUB-6.
- **LOG-F-16** If `log` holds or receives two validly signed entries of one author that cannot both be in one chain, then `log` shall mark the channel compromised, keep both entries as proof, and store no further entry of that channel. Traces: F-PUB-7, J4.
- **LOG-F-17** While the own channel is compromised, `log` shall refuse each append. Traces: F-PUB-7.
- **LOG-F-18** `log` shall never change or delete a stored entry or a fork proof. Traces: F-PUB-2.
- **LOG-F-19** `log` shall give the entries of a channel from a position, in order and as their exact signed bytes, and the head and the status of a channel. Traces: F-PUB-3, F-PUB-6, F-FIND-4.
- **LOG-F-20** If a stored entry fails the strict rule when `log` reads it, then `log` shall fail that read, change nothing, and name the cause. Traces: F-QUAL-3.
- **LOG-F-21** `log` shall do every function with no network. Traces: F-QUAL-1.

## Step 4: Find

No new function. `dist` reads the status of each channel that an index
names (F-FIND-4) and reports a compromised one (J4) through LOG-F-16 and
LOG-F-19. New statement kinds of an index (W15) need no change in `log`.

## Step 8: Own machines

A replica among the participant's own machines holds no identity (spec
W33, F-OWN-3).

- **LOG-F-22** When the caller opens a directory with no identity, `log` shall create no key, and shall read, verify and store channels as with an identity. Traces: F-OWN-3, J8.
- **LOG-F-23** If a directory opened with no identity holds a key file, then `log` shall refuse the open, name the file, and leave it unchanged. Traces: F-OWN-3, F-QUAL-3.
- **LOG-F-24** While `log` holds no identity, it shall refuse each signature, backup and restore, and name the cause. Traces: F-OWN-3, W33.
- **LOG-F-25** When the caller deletes an identity that signed no entry, `log` shall remove its key, and hold no identity from then on. If the identity signed an entry, then `log` shall refuse and keep it. Traces: F-OWN-3, F-PUB-7.
- **LOG-F-26** `log` shall list the authors of every stored channel. Traces: F-OWN-4.

## Acceptance scenarios

Each scenario is one test. "A fresh directory" is an empty temporary
directory.

### Identity

- **S1** (LOG-F-1, LOG-F-2) Given a fresh directory, when the caller opens it, then the directory has mode 0700, the key file has mode 0600 and 32 bytes, and a second open gives the same public key.
- **S2** (LOG-F-3) Given a key file with mode 0640, when the caller opens the directory, then the open fails with "exposed", names the path and mode 0640, and the file bytes and mode are unchanged.
- **S3** (LOG-F-3) Given a key file that is a symbolic link to a valid key file, when the caller opens, then the open fails with "not a regular file".
- **S4** (LOG-F-3) Given a key file of 31 bytes, when the caller opens, then the open fails with "corrupt key file".
- **S5** (LOG-F-4) Given the RFC 8032 test 1 secret key in the key file, when the caller opens, then the public key equals the RFC 8032 test 1 public key, and its text equals the z-base-32 form that the test's own encoder gives.
- **S6** (LOG-F-4) Given a key text of 51 or 53 characters, with an upper-case letter, with a letter outside the alphabet, with non-zero unused bits, or of a small-order point, when the caller parses it, then the parse fails.

### Backup

- **S7** (LOG-F-5, LOG-F-6) Given an identity with a backup made with passphrase "correct horse", when the caller opens a fresh directory and restores the backup there with that passphrase, then that directory has the same public key, also after a reopen.
- **S8** (LOG-F-5) Given an identity, when the caller asks for a backup with the empty passphrase, then it fails with "empty passphrase".
- **S9** (LOG-F-7) Given a backup, when the caller restores it with another passphrase, or with one byte changed, then the restore fails with "wrong passphrase or damaged backup", and the key file is unchanged.
- **S10** (LOG-F-7) Given a backup of 107 bytes, another magic, or Argon2id memory above 1 GiB, when the caller restores it, then the restore fails at once with "bad backup" and names the field.
- **S11** (LOG-F-5) Given a passphrase with "é" as one code point, when the caller restores with "é" as "e" and a combining accent, then the restore succeeds.
- **S12** (LOG-F-6) Given a directory whose identity has one entry, when the caller restores another backup into it, then it fails with "identity in use", and the identity is unchanged.
- **S13** (LOG-F-6) Given a directory whose identity has no entry, when the caller restores a backup into it, then the identity of the directory is the identity of the backup.
- **S14** (LOG-F-8) Given a restored identity that has entries 0 to 2 elsewhere, when the caller appends, then it fails with "own head unconfirmed". When the caller then ingests entries 0 to 2 and confirms, the next append has position 3 and links to entry 2.

### Entries

- **S15** (LOG-F-9, LOG-F-11) Given an identity, when the caller appends three entries, then they have positions 0, 1 and 2, each links to the id of the one before, and each verifies with an independent Ed25519 implementation over the documented signed bytes.
- **S16** (LOG-F-11, LOG-F-12) Given entry bytes that the test assembles by hand from the documented layout and signs with an independent implementation, when the caller decodes them, then it accepts them, each field equals the input, and the id is the BLAKE3 hash of the bytes.
- **S17** (LOG-F-12) Given each refusal case of `TECHNICAL.md` section 4.6, when the caller decodes it, then the decode fails with the error that the case names.
- **S18** (LOG-F-12) Given a signature with S + L in place of S, a small-order key with an identity R and S = 0, or a non-canonical key encoding, when the caller decodes the entry, then it fails.
- **S19** (LOG-F-9, LOG-F-13) Given a kind of 33 bytes, an upper-case kind, or a body of 65537 bytes, when the caller appends, then it fails and the own channel is unchanged.

### Channels

- **S20** (LOG-F-14) Given entries 0 to 4 of another author in shuffled order, when the caller ingests them, then all five are stored and the head is 4.
- **S21** (LOG-F-15) Given a stored head 1, when the caller ingests entries 3 and 4, then nothing is stored and the gap is 3.
- **S22** (LOG-F-16) Given a stored entry 1, when the caller ingests another validly signed entry 1 of the same author, then the status is compromised with both entries, and the stored entry 1 is unchanged.
- **S23** (LOG-F-16) Given a stored head 1, when the caller ingests an entry 2 whose link is not the id of entry 1, then the status is compromised with entry 1 and that entry 2.
- **S24** (LOG-F-16, LOG-F-18) Given a compromised channel, when the caller ingests its next valid entry, then nothing is stored. After a reopen, the status is still compromised with the same two entries.
- **S25** (LOG-F-17) Given a compromised own channel, when the caller appends, then it fails with "compromised".
- **S26** (LOG-F-18, LOG-F-20) Given a stored entry whose bytes the test changes in the database file, when the caller reads the channel, then the read fails with "corrupt store" and names the author and the position.
- **S27** (LOG-F-14) Given two handles on one directory in two threads that each append 50 entries, then the own channel has positions 0 to 99 with no gap and a valid chain.
- **S28** (LOG-F-14 to LOG-F-16) Given random sequences of ingest calls of valid entries, gaps, duplicates and forks, then the head, the stored entries and the status equal those of the test's own model of a chain.

### No identity (step 8)

- **S29** (LOG-F-22) Given a fresh directory, when the caller opens it with no identity, then it has mode 0700, a store and no key file; entries 0 to 2 of another author ingest and read back with the same bytes.
- **S30** (LOG-F-23) Given a directory with a key file, when the caller opens it with no identity, then the open fails and names the key file, whose bytes and mode are unchanged.
- **S31** (LOG-F-24) Given a directory opened with no identity, when the caller appends, asks for a backup, restores a backup or confirms the own head, then each fails with "holds no identity", and no file changes.
- **S32** (LOG-F-25) Given an identity with no entry, when the caller deletes it, then the key file is gone, an append fails with "holds no identity", and a reopen with no identity succeeds. Given an identity with one entry, the delete fails with "identity in use" and the key file is unchanged.
- **S33** (LOG-F-26) Given stored channels of three authors, when the caller lists the channels, then it gives the three keys in key order.
