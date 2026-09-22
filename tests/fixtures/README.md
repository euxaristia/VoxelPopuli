`legacy-save-v3.vps` is a synthetic save produced by the version 3 encoder before
the 16-bit ID migration. It contains the `sample_save()` state in `src/save.rs`:
inventory and armor durability, player progress, and edits at positive and
negative coordinates. The world import path is the fictitious `example-world`.
Compatibility tests also derive versions 1 and 2 by removing their later fields.
