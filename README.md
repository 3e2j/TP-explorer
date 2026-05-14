# Twilight Princess Modding Toolchain

TP Explorer is a modding toolchain for *The Legend of Zelda: Twilight Princess* (GameCube, GZ2E).

It supports:
- **export**: extract a vanilla ISO into a modder-friendly folder
- **build**: rebuild changed files from a sparse mod folder using `manifest.json`

## Quick usage

```bash
tp-explorer export <iso_path> <output_dir>
tp-explorer build <iso_path> <mod_dir> <output_dir>
tp-explorer build <iso_path> <mod_dir> --iso-output <patched_iso_path>
```

## Mod folder model

A mod folder is sparse. Add only files you want to change; unchanged files come from the base ISO at build time.

```text
my_mod/
├── manifest.json
├── actors/
└── text/
    └── messages.json
```

## Documentation

- `docs/file-structure.md` - mod path layout and ISO mapping
- `docs/formats` - format specific schema docs
- `docs/architecture.md` - high-level architecture

## Scope

GameCube only (GZ2E / NTSC-U). No Wii or HD (Wii U) support.

> [!IMPORTANT]
> You must provide your own game copy. This repository contains no game assets.
