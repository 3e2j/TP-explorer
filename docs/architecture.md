# Architecture Overview

High-level architecture for TPMT.

---

## Purpose

TPMT bridges:
- **Game-side formats** (ISO files, ARC archives, BMG binaries)
- **Modder-side formats** (friendly folder paths, JSON text formats)

Key idea: mod folders are sparse and mapping-driven.

---

## Main commands

### `export`

Reads a vanilla ISO and produces:
- friendly mod folder structure
- decoded editable files (for supported formats)
- generated `manifest.json` mapping file

### `build`

Uses:
- base ISO
- mod folder edits
- `manifest.json`

to rebuild only changed outputs (including archive repacks) and optionally emit a patched ISO.

---

## Core data flow

1. Parse ISO file table.
2. Unpack ARC files (including nested archives).
3. Decode known formats for editing (for example BMG -> JSON).
4. Generate `manifest.json` linking mod paths back to ISO/archive targets.
5. On build, hash-compare mod files vs manifest baseline.
6. Recompile edited formats (for example JSON -> BMG).
7. Write direct files to the output folder, then repack affected archives.
8. Optionally rebuild the ISO using both direct file replacements and rebuilt archives.

---

## Key components

- `src/commands/export/` - export pipeline, manifest generation, consolidated text export
- `src/commands/build/` - hash check, compile stage, archive assembly
- `src/formats/` - format specific parsing/rebuild helpers

---

## Important design constraints

- Manifest-driven mapping is authoritative.
- Unchanged files should pass through from base ISO untouched.
- Errors should be explicit and actionable.
- Output should be deterministic for identical inputs.

---

## Related docs

- `file-structure.md` - user-facing path mapping
- `formats/manifest.md` - mapping schema
