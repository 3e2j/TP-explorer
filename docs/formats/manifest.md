# manifest.json format

`manifest.json` is a generated mapping file used by the build pipeline.

For normal modding, you should **not edit it by hand**.  
Edit files in your mod folder; the tool uses the manifest to map those edits back to ISO paths.

---

## What this file is for

`manifest.json` tells the toolchain:

- where each mod-facing file canonically lives in the ISO
- whether extracted files canonically live within a archive file `.arc`
- what base hash was exported (for detecting changes)
- how `text/messages.json` maps back to multiple source `.bmg` files

Path layout and folder naming are documented elsewhere (`../file-structure.md`).  
This page only describes the manifest schema.

### Modder notes

- Keep `manifest.json` in your mod folder.
- Do not rename or delete entries manually.
- If mappings look wrong, re-run export from a clean base ISO rather than hand-editing manifest data.

---

## Top-level shape

```json
{
  "version": 1,
  "game": {
    "id": "GZ2E",
    "region": "NTSC-U",
    "platform": "gamecube"
  },
  "arcs": ["files/res/Object/Alink.arc"],
  "entries": {}
}
```

### Fields

| Key | Type | Meaning |
|---|---|---|
| `version` | number | Manifest schema version (`1`) |
| `game` | object | Target game identity |
| `arcs` | string[] | All discovered `.arc` files (ISO-style paths) |
| `entries` | object | Map of mod-relative path -> entry object |

---

## `entries` types

Each key in `entries` is a mod-relative path (example: `stages/.../room.dzr`).

### 1) Archive file entry

For a file that lives inside an `.arc`:

```json
"stages/dungeons/forest_temple/R00_00/room.dzr": {
  "archive": "files/res/Stage/D_MN05/R00_00.arc",
  "path": "room.dzr",
  "sha1": "..."
}
```

| Field | Meaning |
|---|---|
| `archive` | ISO path to the containing `.arc` |
| `path` | Internal file path inside that archive |
| `sha1` | Export-time base hash for change detection |

### 2) Direct ISO file entry

For a file stored directly on disc:

```json
"sys/main.dol": {
  "iso": "sys/main.dol",
  "sha1": "..."
}
```

| Field | Meaning |
|---|---|
| `iso` | ISO-relative file path |
| `sha1` | Export-time base hash for change detection |

### 3) Multi-source entry (`text/messages.json`)

For consolidated BMG text:

```json
"text/messages.json": {
  "sources": [
    {
      "archive": "files/res/Msgus/bmgres.arc",
      "path": "zel_00.bmg",
      "sha1": "..."
    }
  ]
}
```

`sources[]` maps each source `.bmg` back to its archive and stores per-source hash metadata.

---

## Advanced implementation notes

This section groups the white-box details in one place.

1. `arcs` includes top-level and nested archive paths discovered during export.
2. `entries` hashes are SHA-1 strings under `sha1`.
3. For direct ISO files, `iso` paths are stored relative to `files/` when applicable (for example `sys/main.dol`).
4. Build currently relies mostly on `entries` (especially `archive`, `path`, `sources`, `sha1`) for change detection and archive reassembly.
5. `text/messages.json` is handled specially: hashes are compared and rebuilt per source entry, not as one monolithic output hash.
