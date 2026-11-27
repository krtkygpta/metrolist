# Icons

The control glyphs are Google's [Material Icons](https://github.com/google/material-design-icons) — `play_arrow`,
`pause`, `skip_previous`, `skip_next`, `volume_up`, `volume_down`, `queue_music`, `arrow_back` — licensed Apache-2.0.
Each was rendered from the **rounded** theme's 24 px SVG (`src/<category>/<name>/materialiconsround/24px.svg`, the
`materialiconsround` folder, not `materialicons`) to a PNG cropped to its ink, at the size the watch draws it.

`scripts/store build` (and `python -m wp2apps.bitmaps apps/metrolist`) turns them into `src/bitmaps.gen.rs`, which
the crate includes with `cmf_wp2_app::bitmaps!()`; see the SDK's `docs/bitmap-assets.md`.
