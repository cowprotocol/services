# mtime cache

Vendored from [denoland/deno](https://github.com/denoland/deno/tree/main/.github/mtime_cache)
(MIT licence). `action.js` and `action.yml` are unchanged.

Cargo decides that a workspace crate is fresh when its source file is older than
the build artifact. A checkout gives every file the current time, thus cargo
rebuilds every workspace crate. This action gives a file its previous time again,
but only when the content is the same. It reads the git blob hash of each tracked
file (`git ls-files --stage --eol`) and keeps a map from hash to time in
`.mtime-cache-db.json`, in the directory given by `cache-path`.

A file whose content changed gets the current time, thus cargo always rebuilds it.
This is the difference to `git-restore-mtime`, which uses the commit date: with
commit dates a tag can look older than an artifact that main built from different
source, and cargo then reuses an incompatible artifact.

Keep `cache-path` out of `target/`: `Swatinem/rust-cache` deletes every loose file
in `target/`, except `CACHEDIR.TAG`.

The build must not write to tracked files. The action records the content hash of
each file before the build, thus a file that the build changes makes the record
disagree with the artifact. The repository has no build scripts today, thus this
cannot occur. Keep it in mind if you add a `build.rs`, or a step that generates
code into the tree.
