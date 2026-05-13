# Twilight Princess Modding Toolchain

A modding API and toolchain for *The Legend of Zelda: Twilight Princess* (GameCube, GZ2E).

## What it does

The toolchain lets you extract game files from a vanilla ISO, edit them, and recompile into a mod folder — without needing to understand the raw disc or archive format.

Three operations are exposed as a library API (with CLI wrappers):

- **Extract** — unpacks a vanilla ISO into a clean, human-readable folder structure
- **Build** — takes a mod folder and a vanilla ISO, patches only the changed files, and builds into a mod folder
- **Diff** — compares a mod folder against the vanilla ISO to show exactly what has changed

## How mods work

A mod is a sparse folder. You only include the files you want to change. Everything else is sourced from the base ISO at build time.

```
my_mod/
├── manifest.json
├── actors/
│   └── enemies/
│       └── darknut/        ← only what you changed
└── text/
    └── bmgres.arc/
        └── zel_00.json
```

This gets built into a mod structure replicating the original game formats ready for ISO patching.

## Scope

GameCube (GZ2E / NTSC-U) only. No Wii or HD (Wii U) support.
