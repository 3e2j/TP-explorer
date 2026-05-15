# manifest.json format

manifest.json is a generated mapping used by the build pipeline. Do not edit it by hand - export regenerates it from the base ISO.

## Purpose

The manifest maps mod-facing (friendly) file paths to their original ISO/ARC locations and export-time hashes. The build uses this to detect changed files and to reassemble archives.

Key points for modders

- Keep manifest.json in your mod folder.
- Do not edit entries manually; re-run export from a clean ISO if mappings look wrong.
- The manifest is primarily for the toolchain; modders rarely need to read or edit it.

## Top-level schema

```json
{
  "version": 1,
  "game": { "id": "GZ2E", "region": "NTSC-U", "platform": "gamecube" },
  "archives": { "files/res/Some.arc": { "friendly/path": { /* entry */ } } },
  "entries": { "friendly/path": { /* entry */ } }
}
```

- `archives` (object): hoisted map of archive ISO path -> object of friendly_path -> entry. Each per-archive entry is the same as the top-level `entries` one, but without an `archive` field. This is the authoritative place the build uses to discover which files belong to each archive.
- `entries` (object): sanitized map of friendly_path -> entry. Entries omit the redundant `archive` fields to keep the top-level list compact.

## Entry types

1) Archive-contained file

```json
{
  "stages/.../room.dzr": {
    "path": "room.dzr",
    "sha1": "..."
  }
}
```

- `path`: internal path inside the archive
- `sha1`: export-time base hash for change detection

2) Direct ISO file

```json
{
  "sys/main.dol": {
    "iso": "sys/main.dol",
    "sha1": "..."
  }
}
```

- `iso`: ISO-relative file path
- `sha1`: export-time base hash

3) Consolidated text (`text/messages.json`)

```json
{
  "text/messages.json": {
    "sources": [
      { "path": "zel_00.bmg", "sha1": "..." }
    ]
  }
}
```

- `sources[]` maps each BMG back to its archive via the hoisted `archives` map. Each source includes `path` and `sha1`.

## Implementation notes (brief)

- The build reads `archives` to find which files belong to an archive; `entries` is a compact canonical listing used for per-file lookups.
- When rebuilding archives, the tool prefers to repackage from modifications alone when the manifest proves all internal files are present; otherwise it fetches the original archive from the ISO and patches it.
