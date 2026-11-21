# 2026-11-21: a test that corrupts "the stored copy" by file size hit a temporary

Record, not a contract. `causes::ac2_a_bad_copy_on_the_server_*` failed about 1 run in 25 under
load, in two shapes: `fetch` returned `Ok` with the good package, or the test itself panicked
with "No such file or directory". The product was right; the test was wrong. It waited for "a
file of the package's size exists under the server's home" and then overwrote every such file.
While a machine fetches a package it holds the bytes in `fetch-<pid>-<n>.tmp` in its home, and
the holder writes its cache file through a same-size temporary beside it, before the file
`dist/packages/<id>` exists. When the wait matched such a temporary, the test overwrote it (the
bytes were already read, or the file was already gone) and the real copy was stored good
afterwards, so the light machine read good bytes; or the file vanished between listing and
writing. Bytes that do not match their hash were never accepted: the fetch re-hashes what it
received, and `store_package` re-hashes before it stores. Evidence: the two failing logs of a
loop of six parallel `it causes::` runs (2 failures in 48, panics at the overwrite and at the
`unwrap_err`); the code paths above; after the test waits for and overwrites exactly the
cache file `dist/packages/<id>`, 0 failures in 60 runs. Rule: a test names the file it
corrupts, never finds it by size.
