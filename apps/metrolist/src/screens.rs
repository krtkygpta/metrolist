//! Metrolist Remote's screens, built from the watch's own LVGL widgets in the stock look (`cmf_wp2_app::stock`), the
//! way ui-gallery does: a start screen with the two surfaces, a player that follows the phone's media session, the
//! phone's queue, and the Metrolist browser (the root categories, then a node's folders or songs). Lists are stock
//! focus pages, so the rows follow the stock app list's fisheye (the arc) while the crown or a drag scrolls them.
//! Leaving a screen deletes its page and everything on it.
//!
//! The controls are Google's Material Icons — `play_arrow`, `pause`, `skip_previous`, `skip_next`, `volume_up`,
//! `queue_music` and `arrow_back` — carried in the package as bitmap assets (`bitmaps/*.png`, `cmf_wp2_app::bitmaps!`).
//! A bitmap has no alpha yet (a transparent pixel becomes black), so each glyph sits on a black control whose colour is
//! the plate around it, and the glow reads around them. The player's glow is the artwork's colour, sent by the phone as
//! one RGB565 value.
//!
//! The search screen has the watch's own keyboard: a QWERTY grid of stock buttons (not an LVGL text area, which would
//! need the loader's input device), so every tap is the app's and the finished query goes to the phone as one frame.
//! The keys fill the bottom of the round screen, as on a phone's keyboard, with the query box above them; the phone's
//! five best matches come back as an ordinary list, so choosing one is the list screen's tap.
//!
//! Back is one small round icon button in the top-left corner of every screen that has a way back (the start screen's
//! leaves the app). It is a child of the root, so it stays put when a focus page scrolls under it.

use crate::bitmaps;
use crate::model::*;
use cmf_wp2_app::lv::*;
use cmf_wp2_app::Bitmap;
use cmf_wp2_app::stock::{self, CIRCLE, GREY, ROW_BG, ROW_W, SCREEN, WHITE};

/// Font slots besides the system font (slot 0).
pub const F24: u8 = 1;
/// The heavier face of the headings: the stock set has no bold, and this is its heaviest text face.
pub const F36: u8 = 2;
/// Bytes one widget group can take: flush first when fewer are left.
const GROUP: usize = 620;
/// `Event.y` of an LVGL CLICKED event.
pub const CLICKED: i16 = 7;
/// `Event.y` of an LVGL VALUE_CHANGED event (a slider's or a switch's new value is in `delta`).
pub const VALUE_CHANGED: i16 = 28;
/// The search keyboard's most keys: 26 letters, space, DEL and Search.
pub const KEY_MAX: usize = 32;
/// Key codes that are not a letter: the letter keys carry `0..=25`.
pub const KEY_SPACE: u8 = 26;
pub const KEY_DEL: u8 = 27;
pub const KEY_GO: u8 = 28;

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

// The volume screen: a heading, then a tall vertical slider down the middle of the face, with the note under it.
// An LVGL bar or slider is vertical when its width is under its height (LVGL 8.3, "Bar (lv_bar)"), and a vertical one
// fills from the bottom up.
const VOL_TITLE_Y: i16 = -170;
const VOL_MSG_Y: i16 = 200;
const VOL_SLIDER_Y: i16 = 10;
const VOL_SLIDER_W: i32 = 48;
const VOL_SLIDER_H: i32 = 300;
/// The slider's track: the stock row grey, so the white fill and knob read against it.
const VOL_TRACK: i32 = stock::ROW_BG;
/// The slider's steps. The loader offers the phone's volume keys, not its level, so the app keeps a fifteen-step track
/// of its own (Android's usual media volume range) and presses a key per step it moves.
pub const VOL_STEPS: i32 = 15;
/// The status line under the heading; [`show_volume`] replaces it when the phone refuses a step.
pub const VOL_MSG: &str = "The phone's own volume";

// The search screen: the query box under the Back button, then the keyboard down to the bottom edge: three QWERTY rows
// and a bottom row of DEL, space and Search. The key rows are as wide as the round face allows at their height: the
// top row spans 456 px where the chord is 464, the bottom row 228 where it is 266.
const SEARCH_BOX_Y: i16 = -100;
const SEARCH_BOX_W: i32 = 340;
const SEARCH_BOX_H: i32 = 50;
const SEARCH_BOX_R: i32 = 25;
const QUERY_HINT: &str = "Type a song";
const SEARCH_KEY_W: i32 = 42;
const SEARCH_KEY_H: i32 = 50;
const SEARCH_KEY_GAP: i16 = 4;
/// The keyboard's four rows, as offsets from the screen's centre: the last one ends 42 px above the bottom edge.
const KEY_Y: [i16; 4] = [4, 58, 112, 166];
/// The Search key's red: the stock palette has no red, and the key has to read as the action.
const SEARCH_RED: i32 = 0xC145;

/// The Back button: a small round icon button in the top-left corner, inside the round face.
const BACK_SIZE: i32 = 44;
const BACK_X: i16 = 70;
const BACK_Y: i16 = 58;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Home,
    Controller,
    Queue,
    Volume,
    Browse,
    List,
    Search,
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
    /// The play button's two icons (bitmaps): one flag record each, so the two can never be left showing at once.
    pub pause_icon: Obj,
    pub play_icon: Obj,
    pub hint: Obj,
    /// A fixed status bar at the bottom of a song list (`ROOT` when the screen has none).
    pub status: Obj,
    /// The queue screen's rows, in queue order.
    pub queue_rows: [Obj; MAX_ITEMS],
    pub queue_count: u8,
    /// The volume screen's slider (`ROOT` elsewhere): the app's track over the phone's volume keys.
    pub volume: Obj,
    /// The search screen's keyboard: one handle per key, with its code (a letter's index, or `KEY_SPACE` /
    /// `KEY_DEL` / `KEY_GO`).
    pub keys: [Obj; KEY_MAX],
    pub key_codes: [u8; KEY_MAX],
    pub key_count: u8,
    /// The box that shows what has been typed (and its hint while it is empty).
    pub query: Obj,
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
    pause_icon: ROOT,
    play_icon: ROOT,
    hint: ROOT,
    status: ROOT,
    queue_rows: [ROOT; MAX_ITEMS],
    queue_count: 0,
    volume: ROOT,
    keys: [ROOT; KEY_MAX],
    key_codes: [0; KEY_MAX],
    key_count: 0,
    query: ROOT,
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

/// One Material icon as a bitmap of the package: an `lv_img` whose pixels stay in the app block and are drawn in place
/// (docs/bitmap-assets.md). It is never clickable, so it does not steal its button's tap, and it is centred on `x`, `y`
/// from its parent's middle.
fn icon(b: &mut Batch, flush: Flush, parent: Obj, bm: &Bitmap, x: i16, y: i16) -> Result<Obj, LvError> {
    room(b, flush)?;
    let img = b.image(parent)?;
    b.set_image(img, bm.px, bm.w, bm.h)?;
    b.align(img, Align::Center, x, y)?;
    b.flags(img, &[], &[Flag::Clickable, Flag::Scrollable])?;
    Ok(img)
}

/// A rounded-rectangle button in the app's black with a single Material icon: the Volume and Queue controls. The icon is
/// the whole button (a label beside it said the same thing twice). The black the icon's transparency became is the
/// button's own colour, so no plate shows around the glyph.
fn icon_pill(b: &mut Batch, flush: Flush, parent: Obj, x: i16, y: i16, bm: &Bitmap) -> Result<Obj, LvError> {
    let pill = b.create(Class::Btn, parent)?;
    b.size(pill, PILL_W, PILL_H)?;
    b.align(pill, Align::Center, x, y)?;
    b.style(pill, Prop::Radius, 0, PILL_R)?;
    b.style(pill, Prop::BgColor, 0, stock::BLACK)?;
    b.style(pill, Prop::BgOpa, 0, 255)?;
    b.style(pill, Prop::BgColor, stock::PRESSED, WHITE)?;
    b.style(pill, Prop::BgOpa, stock::PRESSED, 40)?;
    b.style(pill, Prop::BorderWidth, 0, 0)?;
    b.events(pill, &[Ev::Clicked])?;
    icon(b, flush, pill, bm, 0, 0)?;
    Ok(pill)
}

/// The top-left Back button: a round plate with the arrow, a child of the root so a scrolling page never takes it with
/// it. It only goes back; the app reads the tap.
fn back_button(b: &mut Batch, flush: Flush) -> Result<Obj, LvError> {
    let btn = b.create(Class::Btn, ROOT)?;
    b.size(btn, BACK_SIZE, BACK_SIZE)?;
    b.align(btn, Align::TopLeft, BACK_X, BACK_Y)?;
    b.style(btn, Prop::Radius, 0, CIRCLE)?;
    b.style(btn, Prop::BgColor, 0, ROW_BG)?;
    b.style(btn, Prop::BgOpa, 0, 255)?;
    b.style(btn, Prop::BgColor, stock::PRESSED, WHITE)?;
    b.style(btn, Prop::BgOpa, stock::PRESSED, 40)?;
    b.style(btn, Prop::BorderWidth, 0, 0)?;
    b.events(btn, &[Ev::Clicked])?;
    icon(b, flush, btn, &bitmaps::ARROW_BACK, 0, 0)?;
    Ok(btn)
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

/// The start screen: Search first, then the library, then the player. A stock focus page like the app list, so the crown
/// (or a drag) scrolls the three rows round the screen and the centred one is marked; the top-left arrow leaves the app.
pub fn home(b: &mut Batch, flush: Flush) -> Result<Screen, LvError> {
    room(b, flush)?;
    let (page, _bar) = stock::focus_page(b, 0)?;
    opaque(b, page)?;
    // A gap between the rows: the fisheye page lays them out edge to edge, which reads as one block.
    b.style(page, Prop::PadRow, 0, 8)?;
    let mut s = Screen { kind: Kind::Home, page, ..EMPTY };
    heading(b, flush, page, "Metrolist", stock::FOCUS_ROW_H)?;
    let entries: [(&str, &str); 3] = [
        ("Search", "Find a song by name"),
        ("Metrolist", "Quick picks, library"),
        ("Controller", "Playback on your phone"),
    ];
    for (i, (name, detail)) in entries.into_iter().enumerate() {
        room(b, flush)?;
        let row = stock::focus_row(b, page)?;
        // Only the row the fisheye centres (the loader's CHECKED state) gets a plate. Filling every row painted three
        // bars edge to edge, which read as everything being selected at once.
        b.style(row, Prop::BgColor, CHECKED, ROW_BG)?;
        b.style(row, Prop::BgOpa, CHECKED, 255)?;
        b.flags(row, &[Flag::Clickable], &[Flag::Scrollable])?;
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
    s.back_btn = back_button(b, flush)?;
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

/// Shown when the watch's loader has no bitmap assets: it refuses the player's image records before any of the batch
/// runs, so the app says why instead of leaving a black screen.
pub fn notice(b: &mut Batch, flush: Flush) -> Result<Screen, LvError> {
    room(b, flush)?;
    let mut s = Screen { kind: Kind::Home, ..EMPTY };
    let body = page_black(b, ROOT)?;
    s.page = body;
    s.hint = hint(b, flush, body, "Metrolist needs an App Loader from 2026-11-27 for its icons")?;
    Ok(s)
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
    // Dim: the icon keeps its black plate while the button lights up behind it, so a strong flash would show the plate.
    b.style(btn, Prop::BgOpa, stock::PRESSED, 40)?;
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

    s.play = round_btn(b, body, DISC, 0, DISC_Y, stock::BLACK, 255)?;
    s.pause_icon = icon(b, flush, s.play, &bitmaps::PAUSE, 0, 0)?;
    s.play_icon = icon(b, flush, s.play, &bitmaps::PLAY_ARROW, 2, 0)?;

    s.prev = round_btn(b, body, SIDE, -SIDE_X, DISC_Y, stock::BLACK, 255)?;
    icon(b, flush, s.prev, &bitmaps::SKIP_PREVIOUS, 0, 0)?;
    s.next = round_btn(b, body, SIDE, SIDE_X, DISC_Y, stock::BLACK, 255)?;
    icon(b, flush, s.next, &bitmaps::SKIP_NEXT, 0, 0)?;

    s.vol = icon_pill(b, flush, body, -PILL_X, BOTTOM_Y, &bitmaps::VOLUME_UP)?;
    s.queue_btn = icon_pill(b, flush, body, PILL_X, BOTTOM_Y, &bitmaps::QUEUE_MUSIC)?;

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
    s.back_btn = back_button(b, flush)?;
    show_queue(b, &s, m)?;
    Ok(s)
}

/// The volume screen: the phone's own media volume on a slider. The watch cannot read that level back (the loader
/// offers the phone's volume keys, not its value), so the slider is the app's own fifteen-step track: every step it
/// moves presses a key once, and the crown moves the same track.
pub fn volume(b: &mut Batch, flush: Flush, level: u8) -> Result<Screen, LvError> {
    room(b, flush)?;
    let mut s = Screen { kind: Kind::Volume, ..EMPTY };
    let body = page_black(b, ROOT)?;
    s.page = body;
    s.clock = centred(b, body, VOL_TITLE_Y, 300, 40, "Volume", WHITE, Some(F36))?;
    s.hint = centred(b, body, VOL_MSG_Y, 380, 36, VOL_MSG, GREY, Some(F24))?;
    room(b, flush)?;
    let slider = b.create(Class::Slider, body)?;
    b.size(slider, VOL_SLIDER_W, VOL_SLIDER_H)?;
    b.align(slider, Align::Center, 0, VOL_SLIDER_Y)?;
    b.range(slider, 0, VOL_STEPS)?;
    b.value(slider, (level as i32).min(VOL_STEPS), false)?;
    // White on the stock row grey, rather than the stock accent: this is the volume the phone will play at.
    for (part, colour) in [(0, VOL_TRACK), (stock::INDICATOR, WHITE), (stock::KNOB, WHITE)] {
        b.style(slider, Prop::BgColor, part, colour)?;
        b.style(slider, Prop::BgOpa, part, 255)?;
        b.style(slider, Prop::Radius, part, CIRCLE)?;
    }
    b.style(slider, Prop::BorderWidth, 0, 0)?;
    b.events(slider, &[Ev::ValueChanged])?;
    s.volume = slider;
    s.back_btn = back_button(b, flush)?;
    Ok(s)
}

/// The search screen: the typed query and the app's own QWERTY keyboard. The Search key sends the query to the phone,
/// which answers with a list for `SEARCH_NODE`; the list screen draws the five best matches, so a tap plays from them
/// like any other song list.
pub fn search(b: &mut Batch, flush: Flush, query: &str) -> Result<Screen, LvError> {
    room(b, flush)?;
    let mut s = Screen { kind: Kind::Search, ..EMPTY };
    let body = page_black(b, ROOT)?;
    s.page = body;
    room(b, flush)?;
    let field = b.create(Class::Obj, body)?;
    b.size(field, SEARCH_BOX_W, SEARCH_BOX_H)?;
    b.align(field, Align::Center, 0, SEARCH_BOX_Y)?;
    b.style(field, Prop::Radius, 0, SEARCH_BOX_R)?;
    b.style(field, Prop::BgColor, 0, stock::ROW_BG)?;
    b.style(field, Prop::BgOpa, 0, 255)?;
    b.style(field, Prop::BorderWidth, 0, 0)?;
    b.flags(field, &[], &[Flag::Clickable, Flag::Scrollable])?;
    s.query = centred(
        b,
        field,
        0,
        SEARCH_BOX_W - 28,
        40,
        if query.is_empty() { QUERY_HINT } else { query },
        if query.is_empty() { GREY } else { WHITE },
        Some(F24),
    )?;

    let rows = ["qwertyuiop", "asdfghjkl", "zxcvbnm"];
    for (r, letters) in rows.iter().enumerate() {
        let count = letters.len() as i32;
        for (i, ch) in letters.chars().enumerate() {
            let x = ((i as i32 * 2 - (count - 1)) * (SEARCH_KEY_W + SEARCH_KEY_GAP as i32)) / 2;
            let mut glyph = [0u8; 4];
            let label = ch.encode_utf8(&mut glyph);
            let key = search_key(b, flush, body, x as i16, KEY_Y[r], SEARCH_KEY_W, &*label, stock::ROW_BG)?;
            s.keys[s.key_count as usize] = key;
            // The code is the letter's index in the alphabet, not its place in the row: `h` is 7 whatever row it is
            // on (the row index here is what made the bottom rows type the wrong letters).
            s.key_codes[s.key_count as usize] = ch as u8 - b'a';
            s.key_count += 1;
        }
    }
    // DEL, space and Search on one line, in the phone keyboard's order.
    let bottom: [(i16, i32, &str, u8, i32); 3] = [
        (-90, 48, "DEL", KEY_DEL, stock::ROW_BG),
        (-10, 104, "space", KEY_SPACE, stock::ROW_BG),
        (80, 68, "Search", KEY_GO, SEARCH_RED),
    ];
    s.back_btn = back_button(b, flush)?;
    for (x, w, label, code, fill) in bottom {
        let key = search_key(b, flush, body, x, KEY_Y[3], w, label, fill)?;
        s.keys[s.key_count as usize] = key;
        s.key_codes[s.key_count as usize] = code;
        s.key_count += 1;
    }
    Ok(s)
}

/// One key of the search keyboard: a rounded stock button with a label. The app reads the tap, so no text area and no
/// loader input device are involved.
fn search_key(
    b: &mut Batch,
    flush: Flush,
    parent: Obj,
    x: i16,
    y: i16,
    w: i32,
    label: &str,
    fill: i32,
) -> Result<Obj, LvError> {
    room(b, flush)?;
    let key = b.create(Class::Btn, parent)?;
    b.size(key, w, SEARCH_KEY_H)?;
    b.align(key, Align::Center, x, y)?;
    b.style(key, Prop::Radius, 0, 14)?;
    b.style(key, Prop::BgColor, 0, fill)?;
    b.style(key, Prop::BgOpa, 0, 255)?;
    b.style(key, Prop::BgColor, stock::PRESSED, WHITE)?;
    b.style(key, Prop::BgOpa, stock::PRESSED, 60)?;
    b.style(key, Prop::BorderWidth, 0, 0)?;
    b.events(key, &[Ev::Clicked])?;
    let text = b.create(Class::Label, key)?;
    b.text(text, label)?;
    b.style(text, Prop::TextFont, 0, F24 as i32)?;
    b.align(text, Align::Center, 0, 0)?;
    b.flags(text, &[], &[Flag::Clickable, Flag::Scrollable])?;
    Ok(key)
}

/// The query box follows the typed text; the hint stays grey until there is something to search.
pub fn show_query(b: &mut Batch, s: &Screen, query: &str) -> Result<(), LvError> {
    if s.query == ROOT {
        return Ok(());
    }
    let empty = query.is_empty();
    b.text(s.query, if empty { QUERY_HINT } else { query })?;
    b.style(s.query, Prop::TextColor, 0, if empty { GREY } else { WHITE })
}

/// The volume screen follows the app's track, and says why a step did nothing ([`VOL_MSG`] again when it worked).
pub fn show_volume(b: &mut Batch, s: &Screen, level: u8, text: &str) -> Result<(), LvError> {
    if s.volume != ROOT {
        b.value(s.volume, (level as i32).min(VOL_STEPS), false)?;
    }
    if s.hint == ROOT {
        return Ok(());
    }
    b.text(s.hint, text)
}

/// The pause icon while it plays, the play icon while it is paused: one flag record each, so the two can never be left
/// showing at once (with a record per piece, a batch that ran out between them did exactly that).
pub fn show_state(b: &mut Batch, s: &Screen, playing: bool) -> Result<(), LvError> {
    // `flags(set, clear)`. While it plays the pause bars are up, because a tap then pauses it; while it is paused, the
    // triangle is. (Read the other way round the two swap, which is exactly what a reversed button looks like.)
    let hidden = &[Flag::Hidden][..];
    let shown: &[Flag] = &[];
    if s.pause_icon != ROOT {
        let (set, clear) = if playing { (shown, hidden) } else { (hidden, shown) };
        b.flags(s.pause_icon, set, clear)?;
    }
    if s.play_icon != ROOT {
        let (set, clear) = if playing { (hidden, shown) } else { (shown, hidden) };
        b.flags(s.play_icon, set, clear)?;
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

/// The code of the search keyboard's key `handle`, or `None` when it is not a key.
pub fn key_at(s: &Screen, handle: u16) -> Option<u8> {
    s.keys
        .iter()
        .take(s.key_count as usize)
        .position(|k| k.0 == handle && handle != ROOT.0)
        .map(|i| s.key_codes[i])
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
                Kind::Search,
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
                    Kind::Volume => volume(b, &mut |b| l.flush(b), 8).unwrap(),
                    Kind::Search => search(b, &mut |b| l.flush(b), "song").unwrap(),
                };
                assert_eq!(s.page, first, "{kind:?} reuses the freed handles");
                l.flush(b).unwrap();
                // the runtime updates fit a batch too
                if kind == Kind::Controller {
                    show_now(b, &mut |batch| l.flush(batch), &s, &m, true).unwrap();
                    show_state(b, &s, false).unwrap();
                }
                if kind == Kind::Volume {
                    assert_ne!(s.volume, ROOT);
                    assert_ne!(s.back_btn, ROOT);
                    show_volume(b, &s, 9, "No phone media").unwrap();
                    show_volume(b, &s, 2, VOL_MSG).unwrap();
                }
                if kind == Kind::Search {
                    assert_eq!(s.key_count, 29);
                    show_query(b, &s, "a").unwrap();
                    show_query(b, &s, "").unwrap();
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
        assert_eq!(s.volume, ROOT);
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
        assert_eq!(home_screen.row_count, 3);
        assert_eq!(row_at(&home_screen, home_screen.rows[0].0), Some(0));
        assert_eq!(row_at(&home_screen, home_screen.rows[2].0), Some(2));
        assert_ne!(home_screen.back_btn, ROOT);
        assert_eq!(row_at(&home_screen, ROOT.0), None);
        let search_screen = search(&mut b, &mut flush, "").unwrap();
        assert_eq!(search_screen.key_count, 29);
        assert_eq!(key_at(&search_screen, search_screen.keys[0].0), Some(16), "q is 16");
        assert_eq!(key_at(&search_screen, search_screen.keys[1].0), Some(22), "w is 22");
        // row two starts at `a` and row three at `z`, each with its letter's own code
        assert_eq!(key_at(&search_screen, search_screen.keys[12].0), Some(3), "d is 3");
        assert_eq!(key_at(&search_screen, search_screen.keys[15].0), Some(7), "h is 7");
        assert_eq!(key_at(&search_screen, search_screen.keys[19].0), Some(25), "z is 25");
        assert_eq!(key_at(&search_screen, search_screen.keys[26].0), Some(KEY_DEL));
        assert_eq!(key_at(&search_screen, search_screen.keys[28].0), Some(KEY_GO));
        assert_ne!(search_screen.back_btn, ROOT);
        assert_eq!(key_at(&search_screen, ROOT.0), None);
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
