//! Metrolist Remote's screens, built from the watch's own LVGL widgets in the stock look (`cmf_wp2_app::stock`), the
//! way ui-gallery does: a start screen with the two surfaces, a player that follows the phone's media session, the
//! phone's queue, and the Metrolist browser (the root categories, then a node's folders or songs). Lists are stock
//! focus pages, so the rows follow the stock app list's fisheye (the arc) while the crown or a drag scrolls them.
//! Leaving a screen deletes its page and everything on it.
//!
//! The loader's records can draw only rectangles, arcs and labels: the SDK has no image object and the package has no
//! asset slot, so the two shapes still drawn here are the Material Symbols' `play_arrow` and `pause`, with `arrow_back`
//! for the Back pill. Everything else is text: `<` / `>` for the skips and a rounded `Volume` / `Queue` button. The
//! player's glow is the artwork's colour, sent by the phone as one RGB565 value.

use crate::model::*;
use cmf_wp2_app::lv::*;
use cmf_wp2_app::stock::{
    self, CIRCLE, GREY, ROW_BG, ROW_H, ROW_PRESSED, ROW_W, SCREEN, WHITE,
};

/// Font slots besides the system font (slot 0).
pub const F24: u8 = 1;
/// The heavier face of the headings: the stock set has no bold, and this is its heaviest text face.
pub const F36: u8 = 2;
/// Bytes one widget group can take: flush first when fewer are left.
const GROUP: usize = 620;
/// `Event.y` of an LVGL CLICKED event.
pub const CLICKED: i16 = 7;
/// `Event.y` of an LVGL RELEASED event (a slider let go).
pub const RELEASED: i16 = 8;
/// `Event.y` of an LVGL VALUE_CHANGED event (a slider moved; `Event.delta` is the value).
pub const VALUE_CHANGED: i16 = 28;

/// A row's text starts clear of the curved left edge; no icon or art circle sits behind it.
const TEXT_X: i16 = 28;
const TEXT_W: i32 = ROW_W - TEXT_X as i32 - 28;

// The player, as offsets from the screen centre: the clock, the track and its artist above, the play button on the
// screen's centre point inside the progress ring, the skips beside it and a bottom row of volume and queue.
const DISC: i32 = 150;
const RING: i32 = 172;
const RING_W: i32 = 6;
const DISC_Y: i16 = 0;
const SIDE: i32 = 72;
const SIDE_X: i16 = 156;
const BOTTOM_Y: i16 = 168;
// The clock, the track and its artist share the band above the ring: the ring's top edge is -86 from the centre and
// the play disc's -75, so the text ends well clear of the button that used to cover it.
const CLOCK_Y: i16 = -218;
const TITLE_Y: i16 = -165;
const TITLE_H: i32 = 52;
const ARTIST_Y: i16 = -124;
const ARTIST_H: i32 = 26;

// Volume and Queue: one rounded-rectangle text button each, side by side under the play button.
const PILL_W: i32 = 118;
const PILL_H: i32 = 54;
const PILL_X: i16 = 64;
const PILL_R: i32 = 20;

// The player's surfaces are see-through, so the artwork's glow reads through them: the play button is the most solid,
// the skips the most open.
const PLAY_OPA: i32 = 200;
const SIDE_OPA: i32 = 110;
const PILL_OPA: i32 = 150;

// The glow behind the player: the artwork's colour as concentric opaque bands, because LVGL here has no radial
// gradient. Stacking see-through circles was a mistake: at 16-bit colour a nearly transparent layer rounds away to
// nothing, so the circles left no glow at all. Bands drawn as full-circle arcs hold the colour exactly.
/// One arc per colour step of the ramp; a dim artwork colour uses fewer, a bright one up to this many.
const GLOW_BANDS: usize = 64;
/// Radius of the outermost and the innermost band: well past the round screen (whose radius is 233), then right behind
/// the play button, so the light fills the whole face instead of dying out halfway.
const GLOW_OUT: i32 = 310;
const GLOW_IN: i32 = 24;
/// How much of the artwork's colour the middle band keeps (0..=256). Kept low: the glow is a backdrop for the text and
/// the ring, not a light of its own.
const GLOW_BRIGHT: u32 = 116;

// The volume screen: a heading, the percent, the stock-looking vertical capsule and the Back pill below it.
const VOL_TITLE_Y: i16 = -200;
const VOL_TEXT_Y: i16 = -150;
const VOL_SLIDER_W: i32 = 168;
const VOL_SLIDER_H: i32 = 270;
const VOL_SLIDER_Y: i16 = 5;

/// The queue screen's Back pill: width, height, distance from the bottom edge.
const BACK_W: i32 = 132;
const BACK_H: i32 = 54;
const BACK_Y: i16 = -10;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Home,
    Controller,
    Queue,
    Volume,
    Browse,
    List,
}

/// What the app reacts to on the current screen; unused handles are `ROOT` (never an event target).
#[derive(Clone, Copy)]
pub struct Screen {
    pub kind: Kind,
    pub page: Obj,
    pub rows: [Obj; MAX_ITEMS],
    /// For a list row: the folder's node id, `NONE` for a song.
    pub row_ids: [u8; MAX_ITEMS],
    pub row_count: u8,
    pub title: Obj,
    /// The title's marquee, shown instead of `title` when the text is wider than its box.
    pub title_roll: Obj,
    pub artist: Obj,
    pub clock: Obj,
    pub progress: Obj,
    pub prev: Obj,
    pub play: Obj,
    pub next: Obj,
    /// The glow's bands behind the player, outermost first (`ROOT` on every other screen).
    pub glow: [Obj; GLOW_BANDS],
    pub vol: Obj,
    pub queue_btn: Obj,
    /// The queue screen's fixed Back pill (`ROOT` elsewhere): it only goes back.
    pub back_btn: Obj,
    /// The play button's two icons, each a group of pieces: showing one and hiding the other is two flag records, and
    /// there is no way for a piece of one to be left showing with the other.
    pub pause_group: Obj,
    pub play_group: Obj,
    pub hint: Obj,
    /// A fixed status bar at the bottom of a song list (`ROOT` when the screen has none).
    pub status: Obj,
    /// The queue screen's rows, in queue order.
    pub queue_rows: [Obj; MAX_ITEMS],
    pub queue_count: u8,
    /// The volume screen's slider and percent label (`ROOT` elsewhere).
    pub volume_slider: Obj,
    pub volume_text: Obj,
}

const EMPTY: Screen = Screen {
    kind: Kind::Home,
    page: ROOT,
    rows: [ROOT; MAX_ITEMS],
    row_ids: [NONE; MAX_ITEMS],
    row_count: 0,
    title: ROOT,
    title_roll: ROOT,
    artist: ROOT,
    clock: ROOT,
    progress: ROOT,
    prev: ROOT,
    play: ROOT,
    next: ROOT,
    glow: [ROOT; GLOW_BANDS],
    vol: ROOT,
    queue_btn: ROOT,
    back_btn: ROOT,
    pause_group: ROOT,
    play_group: ROOT,
    hint: ROOT,
    status: ROOT,
    queue_rows: [ROOT; MAX_ITEMS],
    queue_count: 0,
    volume_slider: ROOT,
    volume_text: ROOT,
};

impl Screen {
    pub const fn empty() -> Screen {
        EMPTY
    }
}

type Flush<'a> = &'a mut dyn FnMut(&mut Batch) -> Result<(), LvError>;

fn room(b: &mut Batch, flush: Flush) -> Result<(), LvError> {
    if b.bytes().len() + GROUP > BUF { flush(b) } else { Ok(()) }
}

/// The fonts, in a batch of their own.
pub fn fonts(b: &mut Batch) -> Result<(), LvError> {
    stock::font(b)?;
    b.font(F24, StockFont::F24M)?;
    b.font(F36, StockFont::F36M)
}

/// Once, first: the system font and white text for everything.
pub fn theme(b: &mut Batch) -> Result<(), LvError> {
    stock::theme(b)
}

/// Deletes the current screen (its page, with every object and its scroll bar, and the fixed Back pill).
pub fn leave(b: &mut Batch, s: &Screen) -> Result<(), LvError> {
    if s.page != ROOT {
        b.del(s.page)?;
    }
    for o in [s.status, s.back_btn] {
        if o != ROOT {
            let _ = b.del(o);
        }
    }
    Ok(())
}

/// The white heading of a page: the stock title's place and size in the heavier face. It is a page child, so on a
/// focus page it scrolls away with the rows and takes the fisheye's arc with them.
fn heading(b: &mut Batch, flush: Flush, page: Obj, text: &str, h: i32) -> Result<Obj, LvError> {
    room(b, flush)?;
    let t = b.create(Class::Label, page)?;
    b.size(t, ROW_W, h)?;
    b.long_mode(t, LongMode::Dot)?;
    b.style(t, Prop::TextAlign, 0, 2)?; // LV_TEXT_ALIGN_CENTER
    b.style(t, Prop::TextColor, 0, WHITE)?;
    b.style(t, Prop::TextFont, 0, F36 as i32)?;
    b.text(t, text)?;
    b.flags(t, &[], &[Flag::Clickable, Flag::Scrollable])?;
    Ok(t)
}

/// One solid native LVGL shape, not clickable: an icon piece never steals its button's tap. The pieces paint the
/// Material Symbols' filled glyph: a 24x24 grid scaled to the button, curves as rounded corners and arcs, slants as a
/// run of one-pixel columns (the loader has no rotated or image object).
fn piece(b: &mut Batch, flush: Flush, parent: Obj, w: i32, h: i32, x: i16, y: i16, radius: i32) -> Result<Obj, LvError> {
    room(b, flush)?;
    let o = b.create(Class::Obj, parent)?;
    b.size(o, w, h)?;
    b.align(o, Align::Center, x, y)?;
    // Bump small radii so the shape-drawn icons read as rounded chips, not sharp rects, but never past what the shape
    // can hold: a radius over half the height turned a 1-px triangle row into a capsule that bulged out of it.
    let r = radius.max(4).min(w.min(h) / 2);
    b.style(o, Prop::Radius, 0, r)?;
    b.style(o, Prop::BgColor, 0, WHITE)?;
    b.style(o, Prop::BgOpa, 0, 255)?;
    b.style(o, Prop::BorderWidth, 0, 0)?;
    b.flags(o, &[], &[Flag::Clickable, Flag::Scrollable])?;
    Ok(o)
}

/// How many shapes the play triangle uses (one per row; the rows its rounded corners cut away stay `ROOT`).
pub const PLAY_BARS: usize = 56;

/// The play triangle: 52 wide, 56 tall, its base on the left and its tip on the right.
const PLAY_W: i32 = 52;
const PLAY_H: i32 = 56;
/// How far its three corners are rounded off, in px.
const PLAY_R: i32 = 9;

fn isqrt(v: i32) -> i32 {
    let v = v.max(0);
    let mut r = 0;
    while r * r <= v {
        r += 1;
    }
    r - 1
}

/// `play_arrow`: one 1-px row per line of the triangle, so the slant steps by a single pixel per row, and the base
/// corners and the tip are cut by circles of [`PLAY_R`] px. Wide columns of blocks cannot look rounded (a 2-px column
/// can only round by 1 px); rows can.
fn play_triangle(b: &mut Batch, flush: Flush, parent: Obj, out: &mut [Obj; PLAY_BARS]) -> Result<(), LvError> {
    let half = PLAY_H / 2;
    let half_w = PLAY_W / 2;
    let base = -half_w;
    for (i, slot) in out.iter_mut().enumerate() {
        let y = i as i32 - half;
        let ay = y.abs();
        // The sharp triangle's slanted edge at this row: from the tip (+half_w at y = 0) down to the base corner
        // (-half_w at |y| = half). It falls twice as fast as the width shrinks, which is what makes it a triangle and
        // not a five-sided shape. Then the tip's round corner cuts in: its centre sits far enough back that the arc is
        // tangent to both slant edges, otherwise the circle would lie outside them and cut nothing.
        let mut tip = half_w - 2 * half_w * ay / half;
        if ay < PLAY_R {
            let tip_cx = half_w - PLAY_R * 14 / 10;
            tip = tip.min(tip_cx + isqrt(PLAY_R * PLAY_R - ay * ay));
        }
        // The two corners where the vertical base meets the slanted edges.
        let mut left = base;
        if ay > half - PLAY_R {
            let d = ay - (half - PLAY_R);
            left = left.max(base + PLAY_R - isqrt(PLAY_R * PLAY_R - d * d));
        }
        let w = tip - left;
        if w < 1 {
            continue;
        }
        *slot = piece(b, flush, parent, w, 1, ((left + tip) / 2) as i16, y as i16, 0)?;
    }
    Ok(())
}

/// A container for one icon: transparent, no padding or border, as big as the play disc so its pieces are placed from
/// its centre. Hiding it hides every piece inside it.
fn icon_group(b: &mut Batch, flush: Flush, parent: Obj) -> Result<Obj, LvError> {
    room(b, flush)?;
    let g = b.create(Class::Obj, parent)?;
    b.size(g, DISC, DISC)?;
    b.align(g, Align::Center, 0, 0)?;
    for p in [
        Prop::PadTop,
        Prop::PadBottom,
        Prop::PadLeft,
        Prop::PadRight,
        Prop::BorderWidth,
        Prop::Radius,
        Prop::BgOpa,
    ] {
        b.style(g, p, 0, 0)?;
    }
    b.flags(g, &[], &[Flag::Clickable, Flag::Scrollable])?;
    Ok(g)
}

/// `pause`: two 14x54 bars with a 22-px gap, their ends fully round, in a group of their own.
fn pause_icon(b: &mut Batch, flush: Flush, parent: Obj) -> Result<Obj, LvError> {
    let g = icon_group(b, flush, parent)?;
    piece(b, flush, g, 14, 54, -11, 0, 7)?;
    piece(b, flush, g, 14, 54, 11, 0, 7)?;
    Ok(g)
}

/// Previous / next: a plain `<` or `>` in the heading face on the round button, not a drawn skip icon.
fn chevron(b: &mut Batch, parent: Obj, forward: bool) -> Result<(), LvError> {
    let l = b.create(Class::Label, parent)?;
    b.text(l, if forward { ">" } else { "<" })?;
    b.style(l, Prop::TextFont, 0, F36 as i32)?;
    b.align(l, Align::Center, 0, -2)?;
    b.flags(l, &[], &[Flag::Clickable, Flag::Scrollable])?;
    Ok(())
}

/// A rounded-rectangle text button in the play disc's own shade: the Volume and Queue controls, which keep the
/// bottom row free of small pixel icons.
fn text_pill(b: &mut Batch, parent: Obj, x: i16, label: &str) -> Result<Obj, LvError> {
    let pill = b.create(Class::Btn, parent)?;
    b.size(pill, PILL_W, PILL_H)?;
    b.align(pill, Align::Center, x, BOTTOM_Y)?;
    b.style(pill, Prop::Radius, 0, PILL_R)?;
    b.style(pill, Prop::BgColor, 0, ROW_BG)?;
    b.style(pill, Prop::BgOpa, 0, PILL_OPA)?;
    b.style(pill, Prop::BgColor, stock::PRESSED, ROW_PRESSED)?;
    b.style(pill, Prop::BgOpa, stock::PRESSED, 255)?;
    b.style(pill, Prop::BorderWidth, 0, 0)?;
    b.events(pill, &[Ev::Clicked])?;
    let text = b.create(Class::Label, pill)?;
    b.text(text, label)?;
    b.style(text, Prop::TextFont, 0, F24 as i32)?;
    b.align(text, Align::Center, 0, 0)?;
    b.flags(text, &[], &[Flag::Clickable, Flag::Scrollable])?;
    Ok(pill)
}

/// The queue screen's Back pill: a stock-looking round button that only goes back.
fn back_pill(b: &mut Batch) -> Result<Obj, LvError> {
    let pill = b.create(Class::Btn, ROOT)?;
    b.size(pill, BACK_W, BACK_H)?;
    b.align(pill, Align::BottomMid, 0, BACK_Y)?;
    b.style(pill, Prop::Radius, 0, CIRCLE)?;
    b.style(pill, Prop::BgColor, 0, ROW_BG)?;
    b.style(pill, Prop::BgOpa, 0, 255)?;
    b.style(pill, Prop::BgColor, stock::PRESSED, ROW_PRESSED)?;
    b.style(pill, Prop::BorderWidth, 0, 0)?;
    b.events(pill, &[Ev::Clicked])?;
    let label = b.create(Class::Label, pill)?;
    b.text(label, "Back")?;
    b.style(label, Prop::TextFont, 0, F24 as i32)?;
    b.align(label, Align::Center, 0, 0)?;
    b.flags(label, &[], &[Flag::Clickable, Flag::Scrollable])?;
    Ok(pill)
}

/// A stock focus row: no icon, a title at `LeftMid(28, -16)`, an optional grey second line at `+20`. The fisheye
/// marks the centred row and moves the text along the arc; the app does not tint it.
fn focus_row(b: &mut Batch, flush: Flush, page: Obj, name: &str, detail: &str) -> Result<Obj, LvError> {
    room(b, flush)?;
    let row = stock::focus_row(b, page)?;
    b.events(row, &[Ev::Clicked])?;
    let t = b.create(Class::Label, row)?;
    b.size(t, TEXT_W, if detail.is_empty() { 44 } else { 40 })?;
    b.long_mode(t, LongMode::Dot)?;
    b.text(t, name)?;
    b.align(t, Align::LeftMid, TEXT_X, if detail.is_empty() { 0 } else { -16 })?;
    b.flags(t, &[], &[Flag::Clickable, Flag::Scrollable])?;
    if !detail.is_empty() {
        let d = b.create(Class::Label, row)?;
        b.size(d, TEXT_W, 30)?;
        b.long_mode(d, LongMode::Dot)?;
        b.style(d, Prop::TextFont, 0, F24 as i32)?;
        b.style(d, Prop::TextColor, 0, GREY)?;
        b.text(d, detail)?;
        b.align(d, Align::LeftMid, TEXT_X, 20)?;
        b.flags(d, &[], &[Flag::Clickable, Flag::Scrollable])?;
    }
    Ok(row)
}

/// The start screen: the two surfaces.
pub fn home(b: &mut Batch, flush: Flush) -> Result<Screen, LvError> {
    room(b, flush)?;
    let (page, _bar) = stock::page(b)?;
    opaque(b, page)?;
    let mut s = Screen { kind: Kind::Home, page, ..EMPTY };
    heading(b, flush, page, "Metrolist", stock::TOP - stock::TITLE_Y - stock::ROW_GAP)?;
    let entries: [(&str, &str); 2] =
        [("Controller", "Playback on your phone"), ("Metrolist", "Quick picks, library")];
    for (i, (name, detail)) in entries.into_iter().enumerate() {
        room(b, flush)?;
        let row = stock::card(b, page, ROW_H)?;
        b.style(row, Prop::BgColor, stock::PRESSED, ROW_PRESSED)?;
        b.flags(row, &[Flag::Clickable], &[Flag::Scrollable])?;
        b.events(row, &[Ev::Clicked])?;
        let t = b.create(Class::Label, row)?;
        b.size(t, TEXT_W, 40)?;
        b.long_mode(t, LongMode::Dot)?;
        b.text(t, name)?;
        b.align(t, Align::LeftMid, TEXT_X, -16)?;
        b.flags(t, &[], &[Flag::Clickable, Flag::Scrollable])?;
        let d = b.create(Class::Label, row)?;
        b.size(d, TEXT_W, 30)?;
        b.long_mode(d, LongMode::Dot)?;
        b.style(d, Prop::TextFont, 0, F24 as i32)?;
        b.style(d, Prop::TextColor, 0, GREY)?;
        b.text(d, detail)?;
        b.align(d, Align::LeftMid, TEXT_X, 20)?;
        b.flags(d, &[], &[Flag::Clickable, Flag::Scrollable])?;
        s.rows[i] = row;
        s.row_count = (i + 1) as u8;
    }
    Ok(s)
}

/// The root categories the phone sent.
pub fn browse(b: &mut Batch, flush: Flush, m: &Model) -> Result<Screen, LvError> {
    room(b, flush)?;
    let (page, _bar) = stock::focus_page(b, 0)?;
    opaque(b, page)?;
    let mut s = Screen { kind: Kind::Browse, page, ..EMPTY };
    s.title = heading(b, flush, page, "Metrolist", stock::FOCUS_ROW_H)?;
    if m.root_count == 0 {
        s.hint = hint(b, flush, page, if m.alive(m.last_rx) { "No categories yet" } else { "Waiting for Metrolist on your phone" })?;
        return Ok(s);
    }
    for (i, cat) in m.root.iter().enumerate().take(m.root_count as usize) {
        s.rows[i] = focus_row(b, flush, page, cat.label(), if cat.is_folder() { "" } else { "Songs" })?;
        s.row_count = (i + 1) as u8;
    }
    Ok(s)
}

/// The open node: folders, or songs that start a queue.
pub fn list(b: &mut Batch, flush: Flush, m: &Model) -> Result<Screen, LvError> {
    room(b, flush)?;
    let (page, _bar) = stock::focus_page(b, 0)?;
    opaque(b, page)?;
    let mut s = Screen { kind: Kind::List, page, ..EMPTY };
    s.title = heading(b, flush, page, m.list_title(), stock::FOCUS_ROW_H)?;
    if !m.list_ready {
        s.hint = hint(b, flush, page, "Loading...")?;
        return Ok(s);
    }
    if m.item_count == 0 {
        s.hint = hint(b, flush, page, match m.list_code {
            1 => "Metrolist could not load this",
            2 => "Metrolist is not ready",
            _ => "Nothing here yet",
        })?;
        return Ok(s);
    }
    status_bar(b, &mut s)?;
    for i in 0..m.item_count as usize {
        let item = &m.items[i];
        let detail = if item.artist().is_empty() { "" } else { item.artist() };
        s.rows[i] = focus_row(b, flush, page, item.title(), detail)?;
        s.row_ids[i] = item.id;
        s.row_count = (i + 1) as u8;
    }
    Ok(s)
}

/// A fixed status line at the bottom of a song list. It shows the play round trip: "Playing..." while the
/// phone works, then its ack or "no answer". Idle until `show_status` fills it.
fn status_bar(b: &mut Batch, s: &mut Screen) -> Result<(), LvError> {
    let bar = b.create(Class::Obj, ROOT)?;
    b.size(bar, SCREEN, 56)?;
    b.align(bar, Align::BottomMid, 0, 0)?;
    b.style(bar, Prop::BgColor, 0, stock::BLACK)?;
    b.style(bar, Prop::BgOpa, 0, 0)?;
    b.style(bar, Prop::BorderWidth, 0, 0)?;
    b.style(bar, Prop::Radius, 0, 0)?;
    b.flags(bar, &[], &[Flag::Clickable, Flag::Scrollable])?;
    let t = b.create(Class::Label, bar)?;
    b.size(t, SCREEN - 40, 40)?;
    b.long_mode(t, LongMode::Dot)?;
    b.style(t, Prop::TextAlign, 0, 2)?; // LV_TEXT_ALIGN_CENTER
    b.style(t, Prop::TextColor, 0, GREY)?;
    b.text(t, "")?;
    b.align(t, Align::Center, 0, 0)?;
    b.flags(t, &[], &[Flag::Clickable, Flag::Scrollable])?;
    s.status = bar;
    s.hint = t;
    Ok(())
}

/// Fills (text non-empty) or clears the song list's fixed status line.
pub fn show_status(b: &mut Batch, s: &Screen, text: &str) -> Result<(), LvError> {
    if s.hint == ROOT || s.status == ROOT {
        return Ok(());
    }
    b.text(s.hint, text)?;
    b.style(s.status, Prop::BgOpa, 0, if text.is_empty() { 0 } else { 255 })
}

fn hint(b: &mut Batch, flush: Flush, page: Obj, text: &str) -> Result<Obj, LvError> {
    room(b, flush)?;
    let l = b.create(Class::Label, page)?;
    b.size(l, ROW_W - 48, 96)?;
    b.long_mode(l, LongMode::Wrap)?;
    b.style(l, Prop::TextFont, 0, F24 as i32)?;
    b.style(l, Prop::TextColor, 0, GREY)?;
    b.style(l, Prop::TextAlign, 0, 2)?;
    b.text(l, text)?;
    b.align(l, Align::Center, 0, 20)?;
    b.flags(l, &[], &[Flag::Clickable, Flag::Scrollable])?;
    Ok(l)
}

// ---- the artwork's colour -----------------------------------------------------------------------------------------

/// `c`'s channels: red and blue 0..=31, green 0..=63.
fn channels(c: u16) -> (u32, u32, u32) {
    (((c >> 11) & 31) as u32, ((c >> 5) & 63) as u32, (c & 31) as u32)
}

fn pack(r: u32, g: u32, b: u32) -> i32 {
    ((r.min(31) << 11) | (g.min(63) << 5) | b.min(31)) as i32
}

/// `c` at `k / 256` of its brightness, as a style value.
fn shade(c: u16, k: u32) -> i32 {
    let (r, g, b) = channels(c);
    pack(r * k / 256, g * k / 256, b * k / 256)
}

/// The progress ring's track: the same colour, dimmer. It is never white, so the ring cannot read as an outline
/// around the play button; without a colour it is the stock row grey.
fn track_colour(c: u16) -> i32 {
    if c == 0 {
        ROW_BG
    } else {
        shade(c, 118)
    }
}

/// Smoothstep of `t` (0..=256) as 0..=256: soft at both ends, so the bands do not read as steps.
fn smooth(t: u32) -> u32 {
    t * t * (768 - 2 * t) / (256 * 256)
}

/// The glow's shade at radius `r`: the artwork's colour at full strength in the middle, black at the outermost band.
/// Half straight ramp, half smoothstep: a plain smoothstep dropped to a fifth of the colour by mid-screen, which read
/// as a small hot spot rather than a glow.
fn glow_shade(c: u16, r: i32) -> i32 {
    let span = (GLOW_OUT - GLOW_IN).max(1);
    let d = (((r - GLOW_IN).max(0)) * 256 / span).min(256) as u32;
    let f = ((256 - d) + (256 - smooth(d))) / 2;
    shade(c, f * GLOW_BRIGHT / 256)
}

/// The artwork's colour behind everything, as opaque arcs from the outside in.
///
/// A fixed grid of equal bands looked stepped, and more bands did not help: the screen stores colours as RGB565, so
/// the ramp only *has* a few dozen distinguishable shades and a grid puts several bands on the same one and then
/// jumps. Here every integer radius is read and all the radii that round to the same RGB565 value are merged into a
/// single arc, so each band begins exactly where the colour really changes and is exactly as wide as that shade lasts.
/// That is the smoothest a band renderer can do at this depth, and it costs fewer objects than a 40-band grid, not
/// more: a dim sleeve uses a handful of bands.
fn glow(
    b: &mut Batch,
    flush: Flush,
    parent: Obj,
    colour: u16,
    out: &mut [Obj; GLOW_BANDS],
) -> Result<(), LvError> {
    let mut r_out = GLOW_OUT;
    for slot in out.iter_mut() {
        if r_out <= GLOW_IN {
            break;
        }
        let c = glow_shade(colour, r_out);
        let mut r_in = r_out - 1;
        while r_in > GLOW_IN && glow_shade(colour, r_in) == c {
            r_in -= 1;
        }
        room(b, flush)?;
        let a = b.create(Class::Arc, parent)?;
        b.size(a, r_out * 2, r_out * 2)?;
        b.align(a, Align::Center, 0, DISC_Y)?;
        b.angles(a, 0, 360, 0)?;
        b.value(a, 0, false)?;
        b.style(a, Prop::ArcWidth, 0, (r_out - r_in).max(1))?;
        b.style(a, Prop::ArcRounded, 0, 0)?;
        b.style(a, Prop::ArcColor, 0, c)?;
        b.style(a, Prop::ArcOpa, 0, 255)?;
        b.style(a, Prop::ArcOpa, stock::INDICATOR, 0)?;
        b.style(a, Prop::BgOpa, stock::KNOB, 0)?;
        b.flags(a, &[], &[Flag::Clickable, Flag::Scrollable])?;
        *slot = a;
        r_out = r_in;
    }
    Ok(())
}

fn page_black(b: &mut Batch, parent: Obj) -> Result<Obj, LvError> {
    let page = b.create(Class::Obj, parent)?;
    b.size(page, SCREEN, SCREEN)?;
    for p in [Prop::PadTop, Prop::PadBottom, Prop::PadLeft, Prop::PadRight, Prop::BorderWidth, Prop::Radius] {
        b.style(page, p, 0, 0)?;
    }
    b.style(page, Prop::BgColor, 0, stock::BLACK)?;
    b.style(page, Prop::BgOpa, 0, 255)?;
    b.flags(page, &[], &[Flag::Scrollable])?;
    // the crown comes to the app (the volume), the page does not scroll
    b.knob(page)?;
    Ok(page)
}

/// A stock page is transparent and relies on the loader's black screen behind it; a screen of this app paints its own
/// black, so one that is left behind (or not yet deleted) can never show through the next.
fn opaque(b: &mut Batch, page: Obj) -> Result<(), LvError> {
    b.style(page, Prop::BgColor, 0, stock::BLACK)?;
    b.style(page, Prop::BgOpa, 0, 255)
}

/// A centred label whose `y` is its own centre on the screen, so a negative `y` sits above the middle.
fn centred(b: &mut Batch, parent: Obj, y: i16, w: i32, h: i32, text: &str, colour: i32, font: Option<u8>) -> Result<Obj, LvError> {
    let l = b.create(Class::Label, parent)?;
    b.size(l, w, h)?;
    b.long_mode(l, LongMode::Dot)?;
    b.style(l, Prop::TextAlign, 0, 2)?;
    b.style(l, Prop::TextColor, 0, colour)?;
    if let Some(f) = font {
        b.style(l, Prop::TextFont, 0, f as i32)?;
    }
    b.text(l, text)?;
    b.align(l, Align::Center, 0, y)?;
    b.flags(l, &[], &[Flag::Clickable, Flag::Scrollable])?;
    Ok(l)
}

fn round_btn(b: &mut Batch, parent: Obj, size: i32, x: i16, y: i16, fill: i32, opa: i32) -> Result<Obj, LvError> {
    let btn = b.create(Class::Btn, parent)?;
    b.size(btn, size, size)?;
    b.align(btn, Align::Center, x, y)?;
    b.style(btn, Prop::Radius, 0, CIRCLE)?;
    b.style(btn, Prop::BgColor, 0, fill)?;
    b.style(btn, Prop::BgOpa, 0, opa)?;
    b.style(btn, Prop::BgColor, stock::PRESSED, WHITE)?;
    b.style(btn, Prop::BgOpa, stock::PRESSED, if opa == 0 { 40 } else { 130 })?;
    b.style(btn, Prop::BorderWidth, 0, 0)?;
    b.events(btn, &[Ev::Clicked])?;
    Ok(btn)
}

/// The player: the phone's current track, play / pause, previous / next, the volume on the crown and the queue. One
/// page; the queue is its own screen ([`queue`]), opened by the bottom row's Queue button.
pub fn controller(b: &mut Batch, flush: Flush, m: &Model, clock: &str) -> Result<Screen, LvError> {
    room(b, flush)?;
    let mut s = Screen { kind: Kind::Controller, ..EMPTY };
    let body = page_black(b, ROOT)?;
    s.page = body;
    // Behind everything: the artwork's colour as a soft glow around the play button, fading into the black page.
    glow(b, flush, body, m.now.colour, &mut s.glow)?;
    s.clock = centred(b, body, CLOCK_Y, 160, 22, clock, GREY, Some(F24))?;
    // One line each: the title centred when it fits and a marquee when it does not, never wrapped (a second line would
    // push the artist down into the play disc).
    s.title = centred(b, body, TITLE_Y, TITLE_BOX, TITLE_H, m.now.title(), WHITE, Some(F36))?;
    s.title_roll = scroller(b, body, TITLE_Y, TITLE_BOX, TITLE_H, m.now.title(), WHITE, F36)?;
    s.artist = centred(b, body, ARTIST_Y, TITLE_BOX, ARTIST_H, m.now.artist(), GREY, Some(F24))?;
    show_track(b, &s, m.now.title())?;

    // The progress ring around the play button: the artwork's colour as a dim track, the played part white. The track
    // is never white, so a fresh track (0 %) does not draw a white circle around the button.
    room(b, flush)?;
    let ring = b.create(Class::Arc, body)?;
    b.size(ring, RING, RING)?;
    b.align(ring, Align::Center, 0, DISC_Y)?;
    b.angles(ring, 0, 360, 270)?;
    b.value(ring, m.now.pos.min(100) as i32, false)?;
    b.style(ring, Prop::ArcWidth, 0, RING_W)?;
    b.style(ring, Prop::ArcRounded, 0, 1)?;
    b.style(ring, Prop::ArcColor, 0, track_colour(m.now.colour))?;
    b.style(ring, Prop::ArcOpa, 0, 255)?;
    b.style(ring, Prop::ArcColor, stock::INDICATOR, WHITE)?;
    b.style(ring, Prop::ArcOpa, stock::INDICATOR, 255)?;
    b.style(ring, Prop::BgOpa, stock::KNOB, 0)?;
    b.flags(ring, &[], &[Flag::Clickable, Flag::Scrollable])?;
    s.progress = ring;

    s.play = round_btn(b, body, DISC, 0, DISC_Y, ROW_BG, PLAY_OPA)?;
    s.pause_group = pause_icon(b, flush, s.play)?;
    s.play_group = icon_group(b, flush, s.play)?;
    let mut rows = [ROOT; PLAY_BARS];
    play_triangle(b, flush, s.play_group, &mut rows)?;

    s.prev = round_btn(b, body, SIDE, -SIDE_X, DISC_Y, ROW_BG, SIDE_OPA)?;
    chevron(b, s.prev, false)?;
    s.next = round_btn(b, body, SIDE, SIDE_X, DISC_Y, ROW_BG, SIDE_OPA)?;
    chevron(b, s.next, true)?;

    s.vol = text_pill(b, body, -PILL_X, "Volume")?;
    s.queue_btn = text_pill(b, body, PILL_X, "Queue")?;

    show_now(b, flush, &s, m, false)?;
    Ok(s)
}

/// The phone's queue: the current entry filled, each row a tap that plays that queue position, and a fixed Back pill
/// at the bottom that only goes back.
pub fn queue(b: &mut Batch, flush: Flush, m: &Model) -> Result<Screen, LvError> {
    room(b, flush)?;
    let (page, _bar) = stock::focus_page(b, 0)?;
    opaque(b, page)?;
    let mut s = Screen { kind: Kind::Queue, page, ..EMPTY };
    s.title = heading(b, flush, page, "Queue", stock::FOCUS_ROW_H)?;
    if m.queue_count == 0 {
        let text = if m.alive(m.last_rx) { "Nothing queued yet" } else { "Waiting for phone" };
        s.hint = hint(b, flush, page, text)?;
    } else {
        for i in 0..m.queue_count as usize {
            let item = &m.queue[i];
            let detail = if item.artist().is_empty() { "" } else { item.artist() };
            let row = focus_row(b, flush, page, item.title(), detail)?;
            s.queue_rows[i] = row;
            s.queue_count = (i + 1) as u8;
        }
    }
    s.back_btn = back_pill(b)?;
    show_queue(b, &s, m)?;
    Ok(s)
}

/// "5%", "50%", "100%".
fn pct_text(level: u8, buf: &mut [u8; 4]) -> &str {
    let v = level.min(100);
    let mut n = 1;
    if v >= 100 {
        buf[..3].copy_from_slice(b"100");
        n = 3;
    } else if v >= 10 {
        buf[0] = b'0' + v / 10;
        buf[1] = b'0' + v % 10;
        n = 2;
    } else {
        buf[0] = b'0' + v;
    }
    buf[n] = b'%';
    core::str::from_utf8(&buf[..n + 1]).unwrap_or("")
}

/// The volume screen: the phone's volume as a stock vertical capsule, the percent above it and a Back pill. A drag
/// previews it and letting go sends it; the crown steps it too.
pub fn volume(b: &mut Batch, flush: Flush, m: &Model) -> Result<Screen, LvError> {
    room(b, flush)?;
    let mut s = Screen { kind: Kind::Volume, ..EMPTY };
    let body = page_black(b, ROOT)?;
    s.page = body;
    s.clock = centred(b, body, VOL_TITLE_Y, 300, 40, "Volume", WHITE, Some(F36))?;
    let mut buf = [0u8; 4];
    s.volume_text =
        centred(b, body, VOL_TEXT_Y, 200, 36, pct_text(m.now.vol, &mut buf), GREY, Some(F36))?;
    room(b, flush)?;
    let slider = b.create(Class::Slider, body)?;
    b.size(slider, VOL_SLIDER_W, VOL_SLIDER_H)?;
    b.align(slider, Align::Center, 0, VOL_SLIDER_Y)?;
    b.range(slider, 0, 100)?;
    b.value(slider, m.now.vol.min(100) as i32, false)?;
    // A tall capsule, taller than wide so LVGL draws it vertically: dark track, white fill, no knob.
    b.style(slider, Prop::BgColor, 0, ROW_BG)?;
    b.style(slider, Prop::BgOpa, 0, 255)?;
    b.style(slider, Prop::Radius, 0, CIRCLE)?;
    b.style(slider, Prop::BgColor, stock::INDICATOR, WHITE)?;
    b.style(slider, Prop::BgOpa, stock::INDICATOR, 255)?;
    b.style(slider, Prop::Radius, stock::INDICATOR, CIRCLE)?;
    b.style(slider, Prop::BgOpa, stock::KNOB, 0)?;
    b.style(slider, Prop::BorderWidth, stock::KNOB, 0)?;
    b.events(slider, &[Ev::ValueChanged, Ev::Released])?;
    s.volume_slider = slider;
    s.back_btn = back_pill(b)?;
    Ok(s)
}

/// The volume screen follows the phone (or a drag): the capsule and its percent.
pub fn show_vol(b: &mut Batch, s: &Screen, level: u8) -> Result<(), LvError> {
    let v = level.min(100);
    if s.volume_slider != ROOT {
        b.value(s.volume_slider, v as i32, false)?;
    }
    if s.volume_text != ROOT {
        let mut buf = [0u8; 4];
        b.text(s.volume_text, pct_text(v, &mut buf))?;
    }
    Ok(())
}

/// The pause bars while it plays, the play triangle while it is paused: one flag record each, so the two can never be
/// left showing at once (with a record per piece, a batch that ran out between them did exactly that).
pub fn show_state(b: &mut Batch, s: &Screen, playing: bool) -> Result<(), LvError> {
    // `flags(set, clear)`. While it plays the pause bars are up, because a tap then pauses it; while it is paused, the
    // triangle is. (Read the other way round the two swap, which is exactly what a reversed button looks like.)
    let hidden = &[Flag::Hidden][..];
    let shown: &[Flag] = &[];
    if s.pause_group != ROOT {
        let (set, clear) = if playing { (shown, hidden) } else { (hidden, shown) };
        b.flags(s.pause_group, set, clear)?;
    }
    if s.play_group != ROOT {
        let (set, clear) = if playing { (hidden, shown) } else { (shown, hidden) };
        b.flags(s.play_group, set, clear)?;
    }
    Ok(())
}

/// The title's box: wide enough for about twenty characters of the heading face.
pub const TITLE_BOX: i32 = 380;
/// A rough advance for one character of F36. Only used to decide whether the title has to scroll and how far the
/// marquee travels: a title this underestimates is ellipsised, one it overestimates scrolls a little and repeats.
const TITLE_CHAR: i32 = 20;

/// The title's width in px, as well as it can be told without measuring glyphs.
pub fn title_width(text: &str) -> i32 {
    (text.chars().count() as i32) * TITLE_CHAR
}

/// True when the title is too wide for its box, so the marquee takes over from the centred one-line label.
pub fn title_rolls(text: &str) -> bool {
    title_width(text) > TITLE_BOX
}

/// The marquee: a window the width of the title's box with a label as wide as the text inside it. [`roll_to`] slides
/// the label and the window cuts off whatever runs past it.
///
/// LVGL's own `SCROLL_CIRCULAR` was tried first and never ran here; a slide the app steps itself needs nothing from
/// the host's animation timer, only the loop the app already has.
fn scroller(
    b: &mut Batch,
    parent: Obj,
    y: i16,
    w: i32,
    h: i32,
    text: &str,
    colour: i32,
    font: u8,
) -> Result<Obj, LvError> {
    let win = b.create(Class::Obj, parent)?;
    b.size(win, w, h)?;
    b.align(win, Align::Center, 0, y)?;
    b.style(win, Prop::BgOpa, 0, 0)?;
    b.style(win, Prop::BorderWidth, 0, 0)?;
    b.style(win, Prop::Radius, 0, 0)?;
    b.flags(win, &[], &[Flag::Clickable, Flag::Scrollable])?;
    // No size is set on the label: it keeps its content width, so the text is never wrapped or cut by the label
    // itself, only by the window around it.
    let l = b.create(Class::Label, win)?;
    b.style(l, Prop::TextColor, 0, colour)?;
    b.style(l, Prop::TextFont, 0, font as i32)?;
    b.text(l, text)?;
    b.align(l, Align::LeftMid, 0, 0)?;
    b.flags(l, &[Flag::Hidden], &[Flag::Clickable, Flag::Scrollable])?;
    Ok(l)
}

/// Slide the marquee `x` px left of its start; 0 shows the beginning of the title.
pub fn roll_to(b: &mut Batch, s: &Screen, x: i16) -> Result<(), LvError> {
    if s.title_roll == ROOT {
        return Ok(());
    }
    b.style(s.title_roll, Prop::TranslateX, 0, -(x as i32))
}

/// The track's title: the centred label when it fits, the marquee when it does not. Only the label that is shown gets
/// the text, and the marquee always starts at the beginning of it.
pub fn show_track(b: &mut Batch, s: &Screen, text: &str) -> Result<(), LvError> {
    let fit = !title_rolls(text);
    if s.title != ROOT {
        if fit {
            b.text(s.title, text)?;
        }
        b.flags(s.title, if fit { &[] } else { &[Flag::Hidden] }, if fit { &[Flag::Hidden] } else { &[] })?;
    }
    if s.title_roll != ROOT {
        if !fit {
            b.text(s.title_roll, text)?;
            roll_to(b, s, 0)?;
        }
        b.flags(s.title_roll, if fit { &[Flag::Hidden] } else { &[] }, if fit { &[] } else { &[Flag::Hidden] })?;
    }
    Ok(())
}

/// `HH:MM`.
pub fn clock_text(hour: u8, minute: u8, buf: &mut [u8; 5]) -> &str {
    *buf = [b'0' + hour / 10 % 10, b'0' + hour % 10, b':', b'0' + minute / 10 % 10, b'0' + minute % 10];
    core::str::from_utf8(buf).unwrap_or("")
}

/// The track, its state and the progress follow the model. `force` rewrites every label.
pub fn show_now(b: &mut Batch, _flush: Flush, s: &Screen, m: &Model, force: bool) -> Result<(), LvError> {
    // The title is not written here: a marquee restarts on every write, and the phone repeats the same title with each
    // progress frame. [`show_track`] is called only when the text really changed.
    if force && s.artist != ROOT {
        b.text(s.artist, m.now.artist())?;
    }
    if s.progress != ROOT {
        b.value(s.progress, m.now.pos.min(100) as i32, false)?;
    }
    show_state(b, s, m.now.playing)
}

/// The queue's playing entry is the filled one (the fisheye's CHECKED alone would move with the scroll).
pub fn show_queue(b: &mut Batch, s: &Screen, m: &Model) -> Result<(), LvError> {
    for i in 0..s.queue_count as usize {
        let on = i == m.queue_current as usize;
        b.style(s.queue_rows[i], Prop::BgColor, 0, ROW_BG)?;
        b.style(s.queue_rows[i], Prop::BgOpa, 0, if on { 255 } else { 0 })?;
    }
    Ok(())
}

pub fn show_title(b: &mut Batch, s: &Screen, m: &Model) -> Result<(), LvError> {
    b.text(s.title, m.list_title())
}

pub fn show_clock(b: &mut Batch, s: &Screen, text: &str) -> Result<(), LvError> {
    if s.clock == ROOT { Ok(()) } else { b.text(s.clock, text) }
}

/// True when `handle` is one of the screen's clickable rows.
pub fn row_at(s: &Screen, handle: u16) -> Option<usize> {
    s.rows.iter().take(s.row_count as usize).position(|r| r.0 == handle && handle != ROOT.0)
}

/// True when `handle` is one of the queue screen's rows.
pub fn queue_at(s: &Screen, handle: u16) -> Option<usize> {
    s.queue_rows.iter().take(s.queue_count as usize).position(|r| r.0 == handle && handle != ROOT.0)
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::boxed::Box;
    use wp2_core::lvcmd::{self, Table};

    struct Loader {
        t: Table,
        live_max: usize,
    }

    impl Loader {
        fn flush(&mut self, b: &mut Batch) -> Result<(), LvError> {
            if b.bytes().is_empty() {
                return Ok(());
            }
            // slots 0 (system), 1 (F24) and 2 (F36) are open
            assert_eq!(lvcmd::validate(b.bytes(), &self.t, 0b0111), Ok(()));
            for r in lvcmd::records(b.bytes()) {
                lvcmd::apply(&mut self.t, &r.unwrap());
            }
            let live = (1..=192u16).filter(|&h| self.t.class(h).is_some()).count();
            self.live_max = self.live_max.max(live);
            b.clear();
            Ok(())
        }
    }

    fn model() -> Model {
        let mut m = Model::new();
        m.connected = true;
        m.apply(
            &[
                b'M', b'1', b'C', 5, 1, KIND_SONGS, 11, b'Q', b'u', b'i', b'c', b'k', b' ', b'p', b'i', b'c', b'k', b's',
                0x34, 0x12, 2, KIND_SONGS, 12, b'L', b'i', b's', b't', b'e', b'n', b' ', b'a', b'g', b'a', b'i', b'n',
                0x45, 0x23, 3, KIND_FOLDER, 9, b'D', b'o', b'w', b'n', b'l', b'o', b'a', b'd', b's', 0x56, 0x34, 4,
                KIND_FOLDER, 7, b'L', b'i', b'b', b'r', b'a', b'r', b'y', 0x67, 0x45, 5, KIND_FOLDER, 7, b'E', b'x', b'p',
                b'l', b'o', b'r', b'e', 0x78, 0x56,
            ],
            1,
        );
        m.push(3, "Downloads");
        let mut f = std::vec![b'M', b'1', b'B', 3, KIND_SONGS, 20, 15];
        f.extend_from_slice(b"Downloaded songs");
        m.apply(&f, 2);
        for start in [0u8, 2] {
            let mut f = std::vec![b'M', b'1', b'I', 3, start, 2];
            for k in 0..2u8 {
                let title = std::format!("Track number {}", start + k);
                f.push(title.len() as u8);
                f.extend_from_slice(title.as_bytes());
                f.push(11);
                f.extend_from_slice(b"Some Artist");
                f.extend_from_slice(&0x1234u16.to_le_bytes());
                f.push(NONE);
            }
            m.apply(&f, 3);
        }
        m.apply(&[b'M', b'1', b'E', 3, 4], 4);
        // a two-entry queue with the second one playing
        m.apply(&[b'M', b'1', b'Q', 2, 1], 5);
        let mut q = std::vec![b'M', b'1', b'J', 0, 2];
        for title in ["Queue song one", "Queue song two"] {
            q.push(title.len() as u8);
            q.extend_from_slice(title.as_bytes());
            q.push(6);
            q.extend_from_slice(b"Artist");
            q.extend_from_slice(&0x1234u16.to_le_bytes());
        }
        m.apply(&q, 6);
        m.now.title[..9].copy_from_slice(b"Song name");
        m.now.tlen = 9;
        m.now.artist[..11].copy_from_slice(b"Artist name");
        m.now.alen = 11;
        m.now.has = true;
        m.now.playing = true;
        m.now.pos = 40;
        m.now.colour = 0x1F3E;
        m
    }

    fn with_loader(f: impl FnOnce(&mut Batch, &mut Loader)) {
        let mut b = Batch::with_buf(Box::leak(Box::new([0; BUF])));
        fonts(&mut b).unwrap();
        let mut l = Loader { t: Table::new(), live_max: 0 };
        l.flush(&mut b).unwrap();
        f(&mut b, &mut l);
        std::println!("most live objects: {}", l.live_max);
        assert!(l.live_max < 190);
    }

    #[test]
    fn every_screen_validates_and_leaving_frees_it() {
        with_loader(|b, l| {
            theme(b).unwrap();
            let mut clock = [0u8; 5];
            let m = model();
            let text = clock_text(9, 30, &mut clock);
            let mut s = home(b, &mut |b| l.flush(b)).unwrap();
            l.flush(b).unwrap();
            let first = s.page;
            for kind in [
                Kind::Browse,
                Kind::List,
                Kind::Controller,
                Kind::Queue,
                Kind::Volume,
                Kind::Home,
            ] {
                leave(b, &s).unwrap();
                l.flush(b).unwrap();
                s = match kind {
                    Kind::Home => home(b, &mut |b| l.flush(b)).unwrap(),
                    Kind::Browse => browse(b, &mut |b| l.flush(b), &m).unwrap(),
                    Kind::List => list(b, &mut |b| l.flush(b), &m).unwrap(),
                    Kind::Controller => controller(b, &mut |b| l.flush(b), &m, text).unwrap(),
                    Kind::Queue => queue(b, &mut |b| l.flush(b), &m).unwrap(),
                    Kind::Volume => volume(b, &mut |b| l.flush(b), &m).unwrap(),
                };
                assert_eq!(s.page, first, "{kind:?} reuses the freed handles");
                l.flush(b).unwrap();
                // the runtime updates fit a batch too
                if kind == Kind::Controller {
                    show_now(b, &mut |batch| l.flush(batch), &s, &m, true).unwrap();
                    show_state(b, &s, false).unwrap();
                }
                if kind == Kind::Volume {
                    assert_ne!(s.volume_slider, ROOT);
                    show_vol(b, &s, 30).unwrap();
                }
                if kind == Kind::Queue {
                    show_queue(b, &s, &m).unwrap();
                }
                if kind == Kind::List {
                    show_title(b, &s, &m).unwrap();
                }
                l.flush(b).unwrap();
            }
        });
    }

    #[test]
    fn the_controller_has_no_rows_and_the_queue_has_a_back_pill() {
        let m = model();
        let mut b = Batch::with_buf(Box::leak(Box::new([0; BUF])));
        let mut flush = |batch: &mut Batch| {
            batch.clear();
            Ok(())
        };
        let s = controller(&mut b, &mut flush, &m, "12:00").unwrap();
        assert_eq!(s.row_count, 0);
        assert_eq!(s.queue_count, 0);
        assert_ne!(s.queue_btn, ROOT);
        assert_eq!(s.back_btn, ROOT);
        let q = queue(&mut b, &mut flush, &m).unwrap();
        assert_eq!(q.kind, Kind::Queue);
        assert_eq!(q.queue_count, 2);
        assert_ne!(q.back_btn, ROOT);
    }

    #[test]
    fn rows_map_back_to_their_handles() {
        let m = model();
        let mut b = Batch::with_buf(Box::leak(Box::new([0; BUF])));
        let mut flush = |batch: &mut Batch| {
            batch.clear();
            Ok(())
        };
        let home_screen = home(&mut b, &mut flush).unwrap();
        assert_eq!(home_screen.row_count, 2);
        assert_eq!(row_at(&home_screen, home_screen.rows[1].0), Some(1));
        assert_eq!(row_at(&home_screen, ROOT.0), None);
        let list_screen = list(&mut b, &mut flush, &m).unwrap();
        assert_eq!(list_screen.row_count, 4);
        assert_eq!(row_at(&list_screen, list_screen.rows[3].0), Some(3));
        let browse_screen = browse(&mut b, &mut flush, &m).unwrap();
        assert_eq!(browse_screen.row_count, 5);
        assert_eq!(browse_screen.row_ids[0], NONE);
        let queue_screen = queue(&mut b, &mut flush, &m).unwrap();
        assert_eq!(queue_at(&queue_screen, queue_screen.queue_rows[1].0), Some(1));
        assert_eq!(row_at(&queue_screen, queue_screen.queue_rows[1].0), None);
    }

    #[test]
    fn waiting_and_empty_lists_show_a_hint() {
        let mut m = Model::new();
        let mut b = Batch::with_buf(Box::leak(Box::new([0; BUF])));
        let mut flush = |batch: &mut Batch| {
            batch.clear();
            Ok(())
        };
        let s = browse(&mut b, &mut flush, &m).unwrap();
        assert_ne!(s.hint, ROOT);
        assert_eq!(s.row_count, 0);
        m.open(7, "Explore");
        let s = list(&mut b, &mut flush, &m).unwrap();
        assert_ne!(s.hint, ROOT);
        m.list_ready = true;
        m.list_code = 1;
        let s = list(&mut b, &mut flush, &m).unwrap();
        assert_ne!(s.hint, ROOT);
    }

    #[test]
    fn clock_and_initial_text() {
        let mut buf = [0u8; 5];
        assert_eq!(clock_text(7, 5, &mut buf), "07:05");
        assert_eq!(clock_text(23, 59, &mut buf), "23:59");
    }
}
