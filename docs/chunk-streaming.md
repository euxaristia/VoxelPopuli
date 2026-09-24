# Chunk streaming performance

Native chunk loading reads terrain and containers on workers. Replacing a pool
slot starts a background save and retains the outgoing chunk until that save
succeeds. If blocks or fluid levels change during the save, the current terrain
is saved again before replacement. Only one displaced-chunk save is pending at
a time. Save errors retain the outgoing terrain and stop further integration.
Final world saving waits for that job before writing the current world state.

During play, at most two completed chunks are integrated per update. Integration,
mesh upload collection and mesh dispatch each yield after a two-millisecond
budget, checked between jobs. A single job can exceed its budget. Initial loading
retains its larger batches. Terrain generation, save format and view distance
are unchanged. Under sustained storage pressure, new terrain arrives later while
movement continues instead of waiting on the database lock.

## Reproduce the CPU profile

```sh
cargo test --release profile_forward_streaming -- --ignored --nocapture
```

The profile creates a temporary native world with seed `1074691402050369410`,
loads a four-chunk radius, then drives 720 forward updates at 0.8 blocks per update.
It crosses enough chunk boundaries to recycle pool slots and drains remaining
jobs before deleting its temporary world. It never opens the player's save.
It measures world-update CPU time without GPU uploads or presentation, so it
is not an end-to-end frame-rate benchmark.

A Windows release run before the fix recorded p95 10.227 ms, p99 29.247 ms,
a maximum of 99.675 ms, and 23 updates over 16 ms. The same route after the fix
recorded p95 0.873 ms, p99 1.082 ms, a maximum of 1.221 ms, and zero updates over
16 ms. Hardware, storage caches and worker scheduling affect these figures.

Regression tests hold the database lock while integrating a chunk, change terrain
and fluid levels during an outgoing save, inject a save failure, and verify
bounded integration and headless mesh completion. The lock regression fails on
the previous synchronous path and passes with worker persistence.
