# File Structure

This document defines the mod folder layout and maps every path to its canonical ISO location.

A mod folder is sparse: include only files you are changing.

Anything missing is sourced from the base ISO at build time.

Direct files are written back to the output folder as-is; archive contents are
repacked into their parent `.arc` files.

---

## Arc-as-Folder Rule

Most `.arc` files are exported as folders named after the arc (without the extension). Internal contents are preserved inside that folder.

```
# ISO:  files/res/Stage/D_MN05/R00_00.arc  (contains bmdr/model.bmd internally)
# Mod:  stages/dungeons/forest_temple/R00_00/bmdr/model.bmd
```

The build tool reverses this mechanically: folder name + `.arc` = original arc filename, folder contents = arc contents.

### Exception: `Msgus` BMG files are consolidated

`files/res/Msgus/*.arc` does **not** follow arc-as-folder in mod output.

Its `.bmg` sources are consolidated into a single editable file: `text/messages.json`

Build splits `messages.json` back into per-archive `.bmg` sources automatically.

---

## Format Conversions

Some files inside `.arc` archives are stored in a binary game format but exported in a more editable form. The toolchain converts them automatically on extract and build.

| Game format | Mod format | Where it appears |
|---|---|---|
| `.bmg` | `.json` | `text/messages.json` (consolidated from `files/res/Msgus/*.arc`) |

When working with these files, always use the mod format. The build tool converts them back before packing.

---

## Top-Level Layout

```
my_mod/
├── manifest.json
├── audio/
├── actors/
├── cutscenes/
├── stages/
├── code/
├── ui/
├── text/
├── res/
├── particles/
├── movie/
└── sys/
```

---

## audio/

Streaming music, sound sequences, and sample wave banks.

```
audio/
├── music/          → files/Audiores/Stream/*.ast
├── sequences/      → files/Audiores/Seqs/Z2SoundSeqs.arc  (contains .bms files)
└── waves/          → files/Audiores/Waves/*.aw
```

Also at the Audiores root (not inside a subdirectory):
- `Z2CSRes.arc` → `files/Audiores/Z2CSRes.arc`
- `Z2Sound.baa` → `files/Audiores/Z2Sound.baa`

---

## actors/

All character, enemy, NPC, and object models. Each entry is a `.arc` file from ISO path `files/res/Object/`.

### actors/link/

| Mod filename | ISO filename | Description |
|---|---|---|
| `link_animations` | `AlAnm.arc` | Nearly all of Link's animations + weapon models |
| `link` | `Alink.arc` | Link's main model |
| `link_sumo` | `alSumou.arc` | Sumo Link |
| `bottle` | `al_bottle.arc` | Empty bottle |

### actors/enemies/

| Mod filename | ISO filename | Description |
|---|---|---|
| `armos` | `E_ai.arc` | Armos |
| `keese` | `E_ba.arc` | Keese |
| `boar` | `E_bb.arc` | Boar |
| `bees` | `E_bee.arc` | Bees |
| `fish_bomb` | `E_bg.arc` | Fish bomb |
| `beamos` | `E_bm6.arc` | Beamos |
| `lizalfos` | `E_dn.arc` | Lizalfos |
| `morpheel` | `E_dt.arc` | Morpheel |
| `fyrus` | `E_fm.arc` | Fyrus |
| `freezard` | `E_fz.arc` | Freezard |
| `aeralfos` | `E_ge.arc` | Aeralfos |
| `gibdo` | `E_gi.arc` | Gibdo |
| `gohma_small` | `E_gm.arc` | Small Gohma |
| `dangoro` | `E_gob.arc` | Dangoro (Goron mini-boss) |
| `hyrule_soldier_ghost` | `E_gs.arc` | Hyrule ghost soldier |
| `lava_slug` | `E_hm.arc` | Lava slug |
| `poe` | `E_hp.arc` | Poe |
| `lanmola` | `E_hz.arc` | Lanmola |
| `lanmola_plate` | `E_hzp.arc` | Lanmola plate |
| `ice_keese` | `E_kk.arc` | Ice Keese |
| `iron_knuckle` | `E_md.arc` | Iron ball enemy |
| `darknut` | `E_mf.arc` | Darknut |
| `armogohma` | `E_mg.arc` | Armogohma |
| `ook` | `E_mk.arc` | Ook |
| `helmasaur` | `E_mm.arc` | Helmasaur |
| `helmasaur_armor` | `E_mm_mt.arc` | Helmasaur armour |
| `bee_nest` | `E_nest.arc` | Bee nest |
| `bulblin` | `E_oc.arc` | Bulblin (red) |
| `bulblin_blue` | `E_oc2.arc` | Bulblin (blue) |
| `tadpole` | `E_ot.arc` | Tadpole |
| `peahat` | `E_ph.arc` | Peahat |
| `skull_kid` | `E_pm.arc` | Skull Kid |
| `poe_arbiter` | `E_po.arc` | Arbiter's Grounds poe |
| `phantom_zant` | `E_pz.arc` | Phantom Zant |
| `leever` | `E_rb.arc` | Leever |
| `bulblin_rider` | `E_rd.arc` | Bulblin rider |
| `king_bulblin` | `E_rdb.arc` | King Bulblin |
| `twilit_bulblin` | `E_rdy.arc` | Twilit Bulblin |
| `shadow_beast_early` | `E_s1.arc` | Shadow Beast (early version) |
| `shadow_beast` | `E_s2.arc` | Shadow Beast |
| `deku_toad` | `E_sb.arc` | Deku Toad |
| `stalfos` | `E_sf.arc` | Stalfos |
| `blob` | `E_sm.arc` | Blob enemy |
| `skulltula` | `E_st.arc` | Skulltula |
| `sandfish` | `E_sw.arc` | Sand fish |
| `darkhammer` | `E_th.arc` | Darkhammer |
| `tektite` | `E_ttb.arc` | Tektite |
| `death_sword` | `E_va.arc` | Death Sword |
| `spider` | `E_ws.arc` | Spider |
| `white_wolf` | `E_ww.arc` | White wolf |
| `diababa` | `E_yb.arc` | Diababa (bug queen) |
| `twilit_kargaroc` | `E_yc.arc` | Twilit Kargaroc |
| `twilit_deku_baba` | `E_yd.arc` | Twilit Deku Baba |
| `twilit_rat` | `E_yg.arc` | Twilit rat |
| `twilit_keese` | `E_yk.arc` | Twilit Keese |
| `twilit_bug` | `E_ym.arc` | Twilit bug |
| `twilit_kargaroc_large` | `E_yr.arc` | Twilit large bird |
| `palace_hand` | `E_zh.arc` | Palace of Twilight hand |
| `zant_head` | `E_zm.arc` | Zant head |
| `dead_soldier` | `E_zs.arc` | Stallord's zombie soldiers |

### actors/npcs/

| Mod filename | ISO filename | Description |
|---|---|---|
| `midna` | `Dmidna.arc` | Midna |
| `zelda` | `Zelda.arc` | Princess Zelda |
| `zelda_cape` | `zelRf.arc` | Zelda with cape |
| `zelda_cape_no_hood` | `zelRo.arc` | Zelda with cape, no hood |
| `zant` | `Zant.arc` | Zant |
| `zant_no_helmet` | `zanB.arc` | Zant (helmet removal anim) |
| `ilia` | `Yelia.arc` | Ilia |
| `ilia_bag` | `yel_bag.arc` | Ilia's bag |
| `ilia_twilight` | `yelB_TW.arc` | Ilia (twilight) |
| `doctor_borville` | `Doc.arc` | Doctor Borville |
| `heros_shade` | `GWolf.arc` | Hero's Shade (gold wolf) |
| `ashei` | `Ash.arc` | Ashei |
| `beth` | `Besu.arc` | Beth |
| `colin` | `Kolin.arc` | Colin |
| `renado` | `Shaman.arc` | Renado |
| `yeto` | `ykM.arc` | Yeto |
| `yeta` | `ykW.arc` | Yeta |
| `zora` | `zrA_MDL.arc` | Generic Zora |
| `prince_ralis` | `zrC_MDL.arc` | Prince Ralis |
| `queen_rutela` | `zrZ_GT.arc` | Queen Rutela |
| `twili` | `yamiT.arc` | Twili person |
| `twili_small` | `yamiS.arc` | Twili person (small) |
| `twili_chubby` | `yamiD.arc` | Twili person (chubby) |

### actors/objects/

Interactable world objects, background geometry, props, and effects. All map to ISO path `files/res/Object/`. Each mod folder listed below corresponds to one `.arc` file of the same name in the ISO.

The table groups related arcs into logical categories to make them easier to navigate. Within each group, each row is a single arc expanded as a folder.

| Mod folder | ISO filename | Description |
|---|---|---|
| `backgrounds/@bg0000` – `backgrounds/@bg0063` | `@bg0000.arc` – `@bg0063.arc` | Background geometry segments |
| `props/<name>` | `A_<name>.arc` | Doors, decorations, misc props |
| `dungeon_props/<name>` | `D_<name>.arc` | Dungeon-specific objects |
| `doors/<name>` | `Door<name>.arc` | Door events and models |
| `flags/FlagObj00` – `flags/FlagObj06` | `FlagObj00.arc` – `FlagObj06.arc` | Flags |
| `items/f_gD_rupy` | `f_gD_rupy.arc` | Rupee model |
| `items/fairy` | `fairy.arc` | Fairy model |
| `items/Zmdl` | `Zmdl.arc` | Item models |
| `effects/<name>` | `ef_<name>.arc`, `efWater.arc`, `glwSphere.arc` | Particle effects |

---

## cutscenes/

Cutscene model and animation packages from ISO path `files/res/Object/Demo*.arc`.

| Mod filename | ISO filenames | Description |
|---|---|---|
| `intro` | `Demo01_*.arc` | Opening, Ordon life |
| `ordon` | `Demo02_00.arc`, `Demo04_*.arc` | Goodbye, kidnapping, twilight transformation |
| `prison` | `Demo06_*.arc` | Wolf Link imprisoned, Midna's hand |
| `zelda` | `Demo07_*.arc` | Meeting Zelda, Zant's invasion |
| `midna` | `Demo08_*.arc`, `Demo20_*.arc`, `Demo21_*.arc`, `Demo23_*.arc` | Midna scenes |
| `spirits` | `Demo09_*.arc`, `Demo11_*.arc` | Light spirits, tears |
| `kakariko` | `Demo13_*.arc` – `Demo17_*.arc`, `Demo35_*.arc` | Kakariko scenes |
| `mirror` | `Demo18_*.arc`, `Demo24_*.arc`, `Demo30_*.arc` | Mirror of Twilight scenes |
| `castle_town` | `Demo19_*.arc` | Telma's bar, cart |
| `master_sword` | `Demo22_*.arc` | Master Sword sequence |
| `zant` | `Demo25_*.arc`, `Demo33_*.arc` | Zant and Midna |
| `ganondorf` | `Demo27_*.arc` – `Demo29_*.arc`, `Demo32_*.arc` | Ganondorf scenes |
| `ending` | `Demo31_*.arc` | All ending scenes |
| `title` | `Demo38_01.arc` | Title screen cutscene |

---

## stages/

Stage geometry, room layouts, and decoration data. Each stage folder contains room files (`R##_00.arc`) and a stage file (`STG_00.arc`).

All stages map from the ISO path `files/res/Stage/`

### Stage prefix key

| Prefix | Meaning |
|---|---|
| `D_MN` | Dungeon (main) |
| `D_SB` | Dungeon (side) - grottos, Cave of Ordeals |
| `F_SP` | Field / overworld area |
| `R_SP` | Interior spot - village buildings, indoor areas |
| `S_MV` | Special / movie stage |

### stages/dungeons/

| Mod filename | ISO directory | Notes |
|---|---|---|
| `forest_temple` | `D_MN05` | |
| `forest_temple/boss` | `D_MN05A` | Diababa |
| `forest_temple/sub_boss` | `D_MN05B` | Ook |
| `goron_mines` | `D_MN04` | |
| `goron_mines/boss` | `D_MN04A` | Fyrus |
| `goron_mines/sub_boss` | `D_MN04B` | Dangoro |
| `lakebed_temple` | `D_MN01` | |
| `lakebed_temple/boss` | `D_MN01A` | Morpheel |
| `lakebed_temple/sub_boss` | `D_MN01B` | Deku Toad |
| `arbiters_grounds` | `D_MN10` | |
| `arbiters_grounds/boss` | `D_MN10A` | Stallord |
| `arbiters_grounds/sub_boss` | `D_MN10B` | Death Sword |
| `snowpeak_ruins` | `D_MN11` | |
| `snowpeak_ruins/boss` | `D_MN11A` | Blizzeta |
| `snowpeak_ruins/sub_boss` | `D_MN11B` | Darkhammer |
| `temple_of_time` | `D_MN06` | |
| `temple_of_time/boss` | `D_MN06A` | Armogohma |
| `temple_of_time/sub_boss` | `D_MN06B` | |
| `city_in_the_sky` | `D_MN07` | |
| `city_in_the_sky/boss` | `D_MN07A` | Argorok |
| `city_in_the_sky/sub_boss` | `D_MN07B` | Aeralfos |
| `palace_of_twilight` | `D_MN08` | |
| `palace_of_twilight/boss` | `D_MN08A` | Zant |
| `palace_of_twilight/sub_boss_b` | `D_MN08B` | |
| `palace_of_twilight/sub_boss_c` | `D_MN08C` | |
| `palace_of_twilight/sub_boss_d` | `D_MN08D` | |
| `hyrule_castle` | `D_MN09` | |
| `hyrule_castle/boss` | `D_MN09A` | |
| `hyrule_castle/ganon_horseback` | `D_MN09B` | |
| `hyrule_castle/ganon_final` | `D_MN09C` | |
| `cave_of_ordeals` | `D_SB01` | 50 rooms |

### stages/fields/

| Mod filename | ISO directory |
|---|---|
| `hyrule_field` | `F_SP121` |
| `hyrule_field_castle_town_entrance` | `F_SP122` |

Remaining `F_SP*` directories are other overworld segments. See `STAGE_NAMES.md` for the full list.

### stages/spots/

| Mod filename | ISO directory | Description |
|---|---|---|
| `ordon_village` | `R_SP00*` | Ordon village areas |
| `kakariko_village` | `R_SP10*` | Kakariko interiors |
| `castle_town` | `R_SP20*` | Castle Town areas |
| `throne_room` | `R_SP301` | Hyrule Castle throne room |

### stages/special/

| Mod filename | ISO directory | Description |
|---|---|---|
| `mirror_chamber` | `S_MV000` | Mirror of Twilight chamber |

---

## code/

Actor behaviour modules and symbol maps.

```
code/
├── rel/    → files/rel/Final/Release/*.rel
└── maps/   → files/map/Final/Release/*.map
```

`.rel` files are the game's dynamically loaded actor modules. Each corresponds to an actor by name (e.g. `d_a_e_fm.rel` = Fyrus, `d_a_npc_zelda.rel` = Zelda). `.map` files are the paired symbol tables.

See `STAGE_NAMES.md` for the full `d_a_npc_*` and `d_a_e_*` name reference.

---

## ui/

HUD, menus, maps, and item icons. All files are from ISO path `files/res/Layout/`.

| Mod filename | ISO filename | Description |
|---|---|---|
| `hud/clctres` | `clctres.arc` | Main HUD (mirror, Midna mask, quest screen) |
| `hud/clctresR` | `clctresR.arc` | HUD (alternate region) |
| `hud/main2D` | `main2D.arc` | Main 2D HUD elements |
| `hud/ringres` | `ringres.arc` | Item ring UI |
| `menus/button` | `button.arc` | Button layout |
| `menus/errorres` | `errorres.arc` | Error screen |
| `menus/optres` | `optres.arc` | Options screen |
| `menus/saveres` | `saveres.arc` | Save screen |
| `menus/skillres` | `skillres.arc` | Skills screen |
| `menus/Title2D` | `Title2D.arc` | Title screen 2D assets |
| `map/fmapres` | `fmapres.arc` | Field map screen icons |
| `map/dmapres` | `dmapres.arc` | Dungeon map screen icons |
| `items/itemicon` | `itemicon.arc` | Item icons |
| `items/itemres` | `itemres.arc` | Item resources |
| `items/itmInfRes` | `itmInfRes.arc` | Item info resources |
| `insects/insectRes` | `insectRes.arc` | Insect collection icons |
| `fish/fishres` | `fishres.arc` | Fish collection icons |
| `file_select` | `files/res/Object/fileSel.arc` | File selection screen |
| `messages/msgcom` | `msgcom.arc` | Common message resources |
| `messages/msgres00` | `msgres00.arc` | Message resources (set 0) |
| `messages/msgres01` | `msgres01.arc` | Message resources (set 1–6) |
| `fonts/fontres` | `files/res/Fontus/fontres.arc` | Game font |
| `fonts/rubyres` | `files/res/Fontus/rubyres.arc` | Ruby font |

---

## text/

Dialogue and script files. All are from ISO path `files/res/Msgus/`.

Msgus `.bmg` sources are merged into `text/messages.json` during extraction. Modders edit `messages.json`, and the build tool splits it back into per-archive `.bmg` sources when rebuilding.

For the exact message/attribute JSON schema and conversion rules, see `formats/messages.md`.

| ISO path | Arc internal file |
|---|---|
| `bmgres.arc` | `zel_00.bmg` + `zel_unit.bmg` (main script) |
| `bmgres1.arc` | `zel_01.bmg` |
| `bmgres2.arc` | `zel_02.bmg` |
| `bmgres3.arc` | `zel_03.bmg` |
| `bmgres4.arc` | `zel_04.bmg` |
| `bmgres5.arc` | `zel_05.bmg` |
| `bmgres6.arc` | `zel_06.bmg` |
| `bmgres7.arc` | `zel_07.bmg` |
| `bmgres8.arc` | `zel_08.bmg` |
| `bmgres99.arc` | Empty |

---

## res/

Game data tables and field map resources from ISO path `files/res/`

| Mod filename | ISO path | Description |
|---|---|---|
| `actor_data/ActorDat.bin` | `ActorDat/ActorDat.bin` | Actor data table |
| `item_tables/enemy_table.bin` | `ItemTable/enemy_table.bin` | Enemy drop table |
| `item_tables/item_table.bin` | `ItemTable/item_table.bin` | Item table |
| `card_icon/cardicon.arc` | `CardIcon/cardicon.arc` | Memory card banner and icon |
| `field_maps/` | `FieldMap/` | Overworld map data (dungeon arcs + Field0.arc) |

---

## particles/

Particle effect files

```
particles/    → files/res/Particle/*.jpc
```

---

## movie/

Pre-rendered video cutscenes.

```
movie/    → files/Movie/*.thp
```

---

## sys/

Low-level disc metadata. Treat as read-only unless you know what you are doing.

| Mod filename | ISO path | Description |
|---|---|---|
| `boot.bin` | `sys/boot.bin` | Disc header |
| `bi2.bin` | `sys/bi2.bin` | Disc info |
| `apploader.img` | `sys/apploader.img` | Apploader |
| `main.dol` | `sys/main.dol` | Main executable |
| `fst.bin` | `sys/fst.bin` | Filesystem table |
