# Evidence Chain

This family records a full evidence chain for one rendered page and proves that
the integrity verifier flips from `valid: true` to `valid: false` after a
deliberate artifact corruption without changing the stored metadata row.

`run.sh` creates snapshot, screenshot, and text artifacts under an isolated
runtime root, records latencies for each operation, captures the pre-corruption
artifact inventory, corrupts the text artifact, and verifies the post-corruption
failure path.
