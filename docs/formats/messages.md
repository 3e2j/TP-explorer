# text/messages.json format

Reference for `.bmg` <-> `.json` under `text/`.

## What you edit

Each BMG JSON file is an array:
1. metadata (`message_count`)
2. messages (`ID`, `attributes`, `text`)
3. optional raw extra sections (`Section`, `Data`)

Each message uses:
- `ID`: `"id, subid"`
- `text`: array of lines
- `attributes`: style/behavior settings

## Attributes (simple rules)

- Mapped fields (`box_style`, `print_style`, `box_position`) use readable named Values (see below).
- Other fields are plain numbers.
- Defaults are omitted automatically to keep JSON compact.
- `flag` attribute is currently unknown; keep it as a raw number if you need to set it.

## Attribute fields

| Key | Meaning |
|---|---|
| `event_label_id` (default) | Event/script linkage value |
| `speaker` | Speaker / voice grouping |
| `box_style` | Dialogue box style |
| `print_style` | Text print behavior |
| `box_position` | Dialogue box position |
| `item_id` | Item-related value |
| `line_arrange` | Line layout mode |
| `sound_mood` | Voice/effort sound mood |
| `camera_id` | Camera behavior selector |
| `anim_base` | Base animation selector |
| `anim_face` | Face animation selector |
| `flag` | Unknown behavior flag |

---

## Attribute Values

### `box_style`

| Name | Description |
|---|---|
| `standard_dialogue` (default) | Standard dialogue box |
| `no_background` | No background |
| `fullscreen_sign_forced_instant` | Fullscreen sign-style, instant text |
| `no_background_voiceless` | No background, no voice |
| `no_background_centered_credits` | No background, centered/credits-like |
| `standard_with_glow_effect` | Standard box with glow |
| `get_item_box` | Item-get box |
| `item_name_or_description` | Item name/description style |
| `header_top_left_area_name` | Top-left header / entering an area |
| `midna_dialogue_blue_text` | Midna dialogue (blue) |
| `animal_wolf_link_green_text` | Animal/Wolf Link (green) |
| `instant_fade_in_non_modal` | Instant fade-in non-modal style |
| `system_message` | System message style |
| `wolf_song_interface` | Wolf song UI style |
| `boss_name_title_card` | Boss title-card style |

### `print_style`

| Name | Description |
|---|---|
| `typewriter_skippable` (default) | Typewriter, skippable |
| `forced_instant_skippable` | Instant, skippable |
| `typewriter_no_skip` | Typewriter, no skip |
| `forced_instant_first_box_fades` | Instant, first box fades |
| `typewriter_no_instant_tag_no_skip` | Typewriter, no instant tag, no skip |
| `typewriter_slow_5x` | Slow typewriter |
| `typewriter_skippable_alt` | Alternate skippable typewriter |
| `typewriter_ui_emphasis` | UI-emphasis typewriter |
| `forced_instant_fade_credits` | Instant with fade/credits behavior |

### `box_position`

| Name | Description |
|---|---|
| `bottom` (default) | Bottom |
| `top` | Top |
| `center` | Center |
| `bottom_alt_mayor_messages` | Alternate bottom (used for wrestling Mayor Bo) |
| `bottom_alt_system_voiceless` | Alternate bottom (system/voiceless) |

## Text

- `text` is an array of lines.
- Control codes stay inline as `{HEX...}`.
- Literal braces are escaped as `\{` and `\}`.

## Advanced notes (tool behavior)

- Attributes are fixed-length in binary (16 bytes).
- Build accepts either structured `attributes` or legacy hex-string attributes.
- Header encodings recognized by the tool: `legacy-bmg`, `windows-1252`, `utf-16be`, `shift-jis`, `utf-8`.
