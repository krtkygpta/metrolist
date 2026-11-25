//! Metrolist Remote: the watch's state and its message protocol. No `Api` in here, so `cargo test` runs it on the
//! computer.
//!
//! The phone (a Metrolist fork with the watch link on) is the only client of YouTube Music: it owns the account, the
//! downloads and the player. The watch only names a node to browse and an item to play.
//!
//! Every frame starts with `M1` and a kind byte; 1..=128 bytes, like every app message.
//!
//! Watch -> phone:
//!   `?`                               hello: app started, resumed or lost the phone; send the state and the root
//!   `b` node                          browse node `0` (root categories) or a folder the phone named
//!   `p` node index                    play item `index` of node `node`; the phone queues the node from there
//!   `t` cmd                           transport: 0 play/pause, 1 next, 2 previous
//!   `v` level                         volume 0..=100
//!   `q`                               send the queue (the controller's second page)
//!   `j` index                         play queue entry `index` of the current queue
//!
//! Phone -> watch:
//!   `S` flags pos vol tlen title alen artist colour:u16le
//!                                     now playing. flags bit 0 playing, bit 2 a track is there (bit 1 the
//!                                     phone's liked state, which the watch does not show);
//!                                     `pos` 0..=100 percent of the track; `colour` the artwork's dominant
//!                                     colour, RGB565, for the player's glow (0 when unknown)
//!   `C` n (id kind nlen name colour:u16le)*
//!                                     the root categories: `kind` 0 opens a folder, 1 is a song list
//!   `B` node kind total tlen title    a list starts; `total` items follow, the watch keeps the first 20
//!   `I` node start n (tlen title alen artist colour:u16le id)*
//!                                     items `start..start+n` of that list; a folder item's `id` is its node
//!   `E` node total                    the list is complete
//!   `X` node code                     no list: 0 empty, 1 error, 2 the phone is not ready
//!   `Q` total current                 the queue starts: `total` entries follow, `current` plays
//!   `J` start n (tlen title alen artist colour:u16le)*
//!                                     queue entries `start..start+n`; the queue is ready at `total`
//!   `H`                               heartbeat (about every 10 s) so the watch can show "Offline"
//!
//! Names: a list title is at most 20 bytes, a category name 20, a track title 34, an artist 20. A two-item `I` frame
//! stays under 128 bytes with those limits, which is why the phone clips them.

pub const MAGIC: [u8; 2] = *b"M1";
pub const MSG_MAX: usize = 128;

pub const MAX_ITEMS: usize = 20;
pub const MAX_ROOT: usize = 6;
pub const DEPTH_MAX: usize = 4;

pub const TITLE_MAX: usize = 34;
pub const ARTIST_MAX: usize = 20;
pub const NAME_MAX: usize = 20;

/// No node, no item id.
pub const NONE: u8 = 0xFF;

pub const HELLO: u8 = b'?';
pub const BROWSE: u8 = b'b';
pub const PLAY: u8 = b'p';
pub const TRANSPORT: u8 = b't';
pub const VOLUME: u8 = b'v';
pub const QUEUE_REQ: u8 = b'q';
pub const QUEUE_PLAY: u8 = b'j';

pub const NOW: u8 = b'S';
pub const CATEGORIES: u8 = b'C';
pub const LIST_BEGIN: u8 = b'B';
pub const ITEMS: u8 = b'I';
pub const LIST_END: u8 = b'E';
pub const LIST_NONE: u8 = b'X';
pub const QUEUE: u8 = b'Q';
pub const QUEUE_ITEMS: u8 = b'J';
pub const HEARTBEAT: u8 = b'H';

/// `K` ack codes: the phone switched to the requested song, refused the command, or did not confirm.
pub const ACK: u8 = b'K';
pub const ACK_PLAYING: u8 = 0;
pub const ACK_REFUSED: u8 = 1;
pub const ACK_NO_SWITCH: u8 = 2;

/// `S` flags.
pub const NOW_PLAYING: u8 = 0x01;
pub const NOW_HAS_TRACK: u8 = 0x04;

/// `C` / `B` node kinds.
pub const KIND_FOLDER: u8 = 0;
pub const KIND_SONGS: u8 = 1;

/// Transport commands.
pub const CMD_TOGGLE: u8 = 0;
pub const CMD_NEXT: u8 = 1;
pub const CMD_PREV: u8 = 2;

/// One crown detent changes the volume by this many percent.
pub const VOL_STEP: i16 = 5;

pub const ROOT_TITLE: &str = "Metrolist";

fn text(bytes: &[u8], len: u8) -> &str {
    core::str::from_utf8(&bytes[..len as usize]).unwrap_or("")
}

#[derive(Clone, Copy)]
pub struct Now {
    pub has: bool,
    pub playing: bool,
    /// 0..=100.
    pub pos: u8,
    /// 0..=100.
    pub vol: u8,
    pub tlen: u8,
    pub title: [u8; TITLE_MAX],
    pub alen: u8,
    pub artist: [u8; ARTIST_MAX],
    /// The artwork's dominant colour, RGB565: the player's glow. 0 when the phone has none.
    pub colour: u16,
}

impl Now {
    pub const EMPTY: Now = Now {
        has: false,
        playing: false,
        pos: 0,
        vol: 50,
        tlen: 0,
        title: [0; TITLE_MAX],
        alen: 0,
        artist: [0; ARTIST_MAX],
        colour: 0,
    };

    pub fn title(&self) -> &str {
        if self.tlen == 0 { "Nothing playing" } else { text(&self.title, self.tlen) }
    }

    pub fn artist(&self) -> &str {
        if self.alen == 0 { "Open Metrolist on your phone" } else { text(&self.artist, self.alen) }
    }
}

#[derive(Clone, Copy)]
pub struct Category {
    pub id: u8,
    pub kind: u8,
    pub nlen: u8,
    pub name: [u8; NAME_MAX],
    pub colour: u16,
}

impl Category {
    pub const EMPTY: Category =
        Category { id: NONE, kind: KIND_SONGS, nlen: 0, name: [0; NAME_MAX], colour: 0x2966 };

    pub fn label(&self) -> &str {
        text(&self.name, self.nlen)
    }

    pub fn is_folder(&self) -> bool {
        self.kind == KIND_FOLDER
    }
}

#[derive(Clone, Copy)]
pub struct Item {
    pub tlen: u8,
    pub title: [u8; TITLE_MAX],
    pub alen: u8,
    pub artist: [u8; ARTIST_MAX],
    /// The track's or album's average colour, RGB565: the row's circle and pill.
    pub colour: u16,
    /// A folder's node id, `NONE` for a song.
    pub id: u8,
}

impl Item {
    pub const EMPTY: Item = Item {
        tlen: 0,
        title: [0; TITLE_MAX],
        alen: 0,
        artist: [0; ARTIST_MAX],
        colour: 0x2966,
        id: NONE,
    };

    pub fn title(&self) -> &str {
        text(&self.title, self.tlen)
    }

    pub fn artist(&self) -> &str {
        text(&self.artist, self.alen)
    }

    pub fn is_folder(&self) -> bool {
        self.id != NONE
    }
}

/// What a frame changed; the device redraws only that.
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Effect {
    pub now: bool,
    pub root: bool,
    pub list: bool,
    /// The list is complete (`E`) or refused (`X`).
    pub done: bool,
    pub heartbeat: bool,
    /// The phone answered a play command (`K`).
    pub ack: bool,
    /// The queue (`Q` / `J`) changed.
    pub queue: bool,
}

pub struct Model {
    pub now: Now,
    pub root: [Category; MAX_ROOT],
    pub root_count: u8,
    /// Bumped on every `C`, so the device rebuilds the category page only when it changed.
    pub root_version: u8,
    pub items: [Item; MAX_ITEMS],
    pub item_count: u8,
    pub list_node: u8,
    pub list_kind: u8,
    pub list_total: u8,
    pub list_ready: bool,
    pub list_code: u8,
    pub list_title: [u8; NAME_MAX],
    pub list_tlen: u8,
    /// The folders opened under the root, for Back.
    path: [u8; DEPTH_MAX],
    path_title: [[u8; NAME_MAX]; DEPTH_MAX],
    path_len: [u8; DEPTH_MAX],
    pub depth: u8,
    pub connected: bool,
    pub last_rx: u32,
    /// Bumped on every accepted `S`: a new track can keep the same percent and play state, so the device
    /// cannot tell a new track from a position update by the fields alone.
    pub now_version: u8,
    /// The code of the last `K` ack: 0 playing, 1 refused, 2 no switch.
    pub ack: u8,
    /// The phone's queue, in queue order (the first `MAX_ITEMS` entries).
    pub queue: [Item; MAX_ITEMS],
    pub queue_count: u8,
    /// How many entries the phone said the queue has (at most `MAX_ITEMS`).
    pub queue_total: u8,
    /// The queue position that plays now.
    pub queue_current: u8,
    pub queue_ready: bool,
    /// Bumped on every queue frame, so the device rebuilds the queue page only when it changed.
    pub queue_version: u8,
}

impl Model {
    pub const fn new() -> Model {
        Model {
            now: Now::EMPTY,
            root: [Category::EMPTY; MAX_ROOT],
            root_count: 0,
            root_version: 0,
            items: [Item::EMPTY; MAX_ITEMS],
            item_count: 0,
            list_node: NONE,
            list_kind: KIND_SONGS,
            list_total: 0,
            list_ready: false,
            list_code: 0,
            list_title: [0; NAME_MAX],
            list_tlen: 0,
            path: [NONE; DEPTH_MAX],
            path_title: [[0; NAME_MAX]; DEPTH_MAX],
            path_len: [0; DEPTH_MAX],
            depth: 0,
            connected: false,
            last_rx: 0,
            now_version: 0,
            ack: 0,
            queue: [Item::EMPTY; MAX_ITEMS],
            queue_count: 0,
            queue_total: 0,
            queue_current: 0,
            queue_ready: false,
            queue_version: 0,
        }
    }

    pub fn list_title(&self) -> &str {
        if self.list_tlen == 0 { ROOT_TITLE } else { text(&self.list_title, self.list_tlen) }
    }

    fn set_list_title(&mut self, src: &[u8]) {
        let len = src.len().min(NAME_MAX);
        self.list_title = [0; NAME_MAX];
        self.list_title[..len].copy_from_slice(&src[..len]);
        self.list_tlen = len as u8;
    }

    /// Opens `node` (a list to load): clears the items and the title until `B` arrives.
    pub fn open(&mut self, node: u8, title: &str) {
        self.list_node = node;
        self.set_list_title(title.as_bytes());
        self.items = [Item::EMPTY; MAX_ITEMS];
        self.item_count = 0;
        self.list_total = 0;
        self.list_ready = false;
        self.list_code = 0;
    }

    /// Opens `node` as a folder inside the current one, remembering the way back.
    pub fn push(&mut self, node: u8, title: &str) {
        self.open(node, title);
        let d = (self.depth as usize).min(DEPTH_MAX - 1);
        self.path[d] = node;
        let len = title.len().min(NAME_MAX);
        self.path_title[d] = [0; NAME_MAX];
        self.path_title[d][..len].copy_from_slice(&title.as_bytes()[..len]);
        self.path_len[d] = len as u8;
        self.depth = (self.depth + 1).min(DEPTH_MAX as u8);
    }

    /// Back out of the open folder: the node to browse again, or `None` on the root categories.
    pub fn pop(&mut self) -> Option<u8> {
        if self.depth == 0 {
            return None;
        }
        self.depth -= 1;
        if self.depth == 0 {
            self.open(0, ROOT_TITLE);
            return Some(0);
        }
        let at = (self.depth - 1) as usize;
        let parent = self.path[at];
        // Copy the parent's title out first: `open` rewrites the model it lives in.
        let mut title = [0u8; NAME_MAX];
        let len = (self.path_len[at] as usize).min(NAME_MAX);
        title[..len].copy_from_slice(&self.path_title[at][..len]);
        let text = core::str::from_utf8(&title[..len]).unwrap_or(ROOT_TITLE);
        self.open(parent, text);
        Some(parent)
    }

    /// The playing track's volume after `delta` crown detents, kept in 0..=100.
    pub fn step_volume(level: u8, delta: i16) -> u8 {
        (level as i16 + delta * VOL_STEP).clamp(0, 100) as u8
    }

    pub fn browse_frame(node: u8) -> [u8; 4] {
        [b'M', b'1', BROWSE, node]
    }

    pub fn play_frame(node: u8, index: u8) -> [u8; 5] {
        [b'M', b'1', PLAY, node, index]
    }

    pub fn transport_frame(cmd: u8) -> [u8; 4] {
        [b'M', b'1', TRANSPORT, cmd]
    }

    pub fn volume_frame(level: u8) -> [u8; 4] {
        [b'M', b'1', VOLUME, level.min(100)]
    }

    pub fn hello_frame() -> [u8; 3] {
        [b'M', b'1', HELLO]
    }

    pub fn queue_frame() -> [u8; 3] {
        [b'M', b'1', QUEUE_REQ]
    }

    pub fn queue_play_frame(index: u8) -> [u8; 4] {
        [b'M', b'1', QUEUE_PLAY, index.min(MAX_ITEMS as u8 - 1)]
    }

    /// The phone answered recently enough to trust the state.
    pub fn alive(&self, now: u32) -> bool {
        self.connected && now.wrapping_sub(self.last_rx) < 25_000
    }

    pub fn mark_disconnected(&mut self) {
        self.connected = false;
    }

    fn take_text(msg: &[u8], p: &mut usize, max: usize, out: &mut [u8]) -> Option<u8> {
        if *p >= msg.len() {
            return None;
        }
        let len = msg[*p] as usize;
        *p += 1;
        if len > max || *p + len > msg.len() {
            return None;
        }
        if core::str::from_utf8(&msg[*p..*p + len]).is_err() {
            return None;
        }
        out[..len].copy_from_slice(&msg[*p..*p + len]);
        *p += len;
        Some(len as u8)
    }

    fn take_u16(msg: &[u8], p: &mut usize) -> Option<u16> {
        if *p + 2 > msg.len() {
            return None;
        }
        let v = u16::from_le_bytes([msg[*p], msg[*p + 1]]);
        *p += 2;
        Some(v)
    }

    fn now_playing(&mut self, msg: &[u8]) -> Effect {
        // flags pos vol tlen title alen artist colour:u16le
        if msg.len() < 10 {
            return Effect::default();
        }
        let flags = msg[3];
        if flags & !0x07 != 0 || msg[4] > 100 || msg[5] > 100 {
            return Effect::default();
        }
        let mut p = 6;
        let mut title = [0u8; TITLE_MAX];
        let tlen = match Self::take_text(msg, &mut p, TITLE_MAX, &mut title) {
            Some(n) => n,
            None => return Effect::default(),
        };
        let mut artist = [0u8; ARTIST_MAX];
        let alen = match Self::take_text(msg, &mut p, ARTIST_MAX, &mut artist) {
            Some(n) => n,
            None => return Effect::default(),
        };
        let colour = match Self::take_u16(msg, &mut p) {
            Some(c) => c,
            None => return Effect::default(),
        };
        if p != msg.len() {
            return Effect::default();
        }
        self.now = Now {
            has: flags & NOW_HAS_TRACK != 0,
            playing: flags & NOW_PLAYING != 0,
            pos: msg[4],
            vol: msg[5],
            tlen,
            title,
            alen,
            artist,
            colour,
        };
        self.now_version = self.now_version.wrapping_add(1);
        Effect { now: true, ..Effect::default() }
    }

    fn categories(&mut self, msg: &[u8]) -> Effect {
        if msg.len() < 4 {
            return Effect::default();
        }
        let n = msg[3] as usize;
        if n > MAX_ROOT {
            return Effect::default();
        }
        let mut p = 4;
        let mut cats = [Category::EMPTY; MAX_ROOT];
        for slot in cats.iter_mut().take(n) {
            if p >= msg.len() {
                return Effect::default();
            }
            let id = msg[p];
            p += 1;
            if p >= msg.len() || msg[p] > KIND_SONGS {
                return Effect::default();
            }
            let kind = msg[p];
            p += 1;
            let mut name = [0u8; NAME_MAX];
            let nlen = match Self::take_text(msg, &mut p, NAME_MAX, &mut name) {
                Some(len) => len,
                None => return Effect::default(),
            };
            let colour = match Self::take_u16(msg, &mut p) {
                Some(c) => c,
                None => return Effect::default(),
            };
            *slot = Category { id, kind, nlen, name, colour };
        }
        if p != msg.len() {
            return Effect::default();
        }
        self.root = cats;
        self.root_count = n as u8;
        self.root_version = self.root_version.wrapping_add(1);
        Effect { root: true, ..Effect::default() }
    }

    fn list_begin(&mut self, msg: &[u8]) -> Effect {
        // node kind total tlen title
        if msg.len() < 7 || msg[3] != self.list_node || msg[4] > KIND_SONGS {
            return Effect::default();
        }
        let total = msg[5];
        let mut p = 6;
        let mut title = [0u8; NAME_MAX];
        let tlen = match Self::take_text(msg, &mut p, NAME_MAX, &mut title) {
            Some(n) => n,
            None => return Effect::default(),
        };
        if p != msg.len() {
            return Effect::default();
        }
        self.items = [Item::EMPTY; MAX_ITEMS];
        self.item_count = 0;
        self.list_kind = msg[4];
        self.list_total = total;
        self.list_title = title;
        self.list_tlen = tlen;
        self.list_ready = total == 0;
        self.list_code = 0;
        Effect { list: true, done: total == 0, ..Effect::default() }
    }

    fn list_items(&mut self, msg: &[u8]) -> Effect {
        // node start n (tlen title alen artist colour:u16le id)*
        if msg.len() < 6 || msg[3] != self.list_node {
            return Effect::default();
        }
        let start = msg[4] as usize;
        let n = msg[5] as usize;
        if n == 0 || start + n > MAX_ITEMS {
            return Effect::default();
        }
        let mut p = 6;
        let mut parsed = [Item::EMPTY; MAX_ITEMS];
        for slot in parsed.iter_mut().take(n) {
            let mut title = [0u8; TITLE_MAX];
            let tlen = match Self::take_text(msg, &mut p, TITLE_MAX, &mut title) {
                Some(len) => len,
                None => return Effect::default(),
            };
            let mut artist = [0u8; ARTIST_MAX];
            let alen = match Self::take_text(msg, &mut p, ARTIST_MAX, &mut artist) {
                Some(len) => len,
                None => return Effect::default(),
            };
            let colour = match Self::take_u16(msg, &mut p) {
                Some(c) => c,
                None => return Effect::default(),
            };
            if p >= msg.len() {
                return Effect::default();
            }
            let id = msg[p];
            p += 1;
            *slot = Item { tlen, title, alen, artist, colour, id };
        }
        if p != msg.len() {
            return Effect::default();
        }
        for (i, item) in parsed.iter().enumerate().take(n) {
            self.items[start + i] = *item;
        }
        self.item_count = self.item_count.max((start + n) as u8);
        Effect { list: true, ..Effect::default() }
    }

    fn queue_begin(&mut self, msg: &[u8]) -> Effect {
        // total current
        if msg.len() != 5 || msg[3] as usize > MAX_ITEMS || msg[4] >= msg[3].max(1) {
            return Effect::default();
        }
        self.queue = [Item::EMPTY; MAX_ITEMS];
        self.queue_count = 0;
        self.queue_total = msg[3];
        self.queue_current = msg[4];
        self.queue_ready = msg[3] == 0;
        self.queue_version = self.queue_version.wrapping_add(1);
        Effect { queue: true, ..Effect::default() }
    }

    fn queue_items(&mut self, msg: &[u8]) -> Effect {
        // start n (tlen title alen artist colour:u16le)*
        if msg.len() < 6 {
            return Effect::default();
        }
        let start = msg[3] as usize;
        let n = msg[4] as usize;
        if n == 0 || start + n > MAX_ITEMS {
            return Effect::default();
        }
        let mut p = 5;
        let mut parsed = [Item::EMPTY; MAX_ITEMS];
        for slot in parsed.iter_mut().take(n) {
            let mut title = [0u8; TITLE_MAX];
            let tlen = match Self::take_text(msg, &mut p, TITLE_MAX, &mut title) {
                Some(len) => len,
                None => return Effect::default(),
            };
            let mut artist = [0u8; ARTIST_MAX];
            let alen = match Self::take_text(msg, &mut p, ARTIST_MAX, &mut artist) {
                Some(len) => len,
                None => return Effect::default(),
            };
            let colour = match Self::take_u16(msg, &mut p) {
                Some(c) => c,
                None => return Effect::default(),
            };
            *slot = Item { tlen, title, alen, artist, colour, id: NONE };
        }
        if p != msg.len() {
            return Effect::default();
        }
        for (i, item) in parsed.iter().enumerate().take(n) {
            self.queue[start + i] = *item;
        }
        self.queue_count = self.queue_count.max((start + n) as u8);
        self.queue_ready = self.queue_total > 0 && self.queue_count >= self.queue_total;
        self.queue_version = self.queue_version.wrapping_add(1);
        Effect { queue: true, ..Effect::default() }
    }

    fn list_end(&mut self, msg: &[u8]) -> Effect {
        if msg.len() != 5 || msg[3] != self.list_node {
            return Effect::default();
        }
        self.list_total = msg[4];
        self.item_count = self.item_count.min(msg[4].min(MAX_ITEMS as u8));
        self.list_ready = true;
        Effect { list: true, done: true, ..Effect::default() }
    }

    fn list_none(&mut self, msg: &[u8]) -> Effect {
        if msg.len() != 5 || msg[3] != self.list_node || msg[4] > 2 {
            return Effect::default();
        }
        self.items = [Item::EMPTY; MAX_ITEMS];
        self.item_count = 0;
        self.list_total = 0;
        self.list_ready = true;
        self.list_code = msg[4];
        Effect { list: true, done: true, ..Effect::default() }
    }

    /// One phone message. A malformed frame changes nothing.
    pub fn apply(&mut self, msg: &[u8], now: u32) -> Effect {
        if msg.len() < 3 || msg[..2] != MAGIC || msg.len() > MSG_MAX {
            return Effect::default();
        }
        let fx = match msg[2] {
            NOW => self.now_playing(msg),
            CATEGORIES => self.categories(msg),
            LIST_BEGIN => self.list_begin(msg),
            ITEMS => self.list_items(msg),
            LIST_END => self.list_end(msg),
            LIST_NONE => self.list_none(msg),
            QUEUE => self.queue_begin(msg),
            QUEUE_ITEMS => self.queue_items(msg),
            HEARTBEAT if msg.len() == 3 => Effect { heartbeat: true, ..Effect::default() },
            ACK if msg.len() == 4 && msg[3] <= ACK_NO_SWITCH => {
                self.ack = msg[3];
                Effect { ack: true, ..Effect::default() }
            }
            _ => Effect::default(),
        };
        if fx != Effect::default() {
            self.connected = true;
            self.last_rx = now;
        }
        fx
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;

    fn now_frame(flags: u8, pos: u8, vol: u8, title: &str, artist: &str, colour: u16) -> std::vec::Vec<u8> {
        let mut f = std::vec![b'M', b'1', NOW, flags, pos, vol, title.len() as u8];
        f.extend_from_slice(title.as_bytes());
        f.push(artist.len() as u8);
        f.extend_from_slice(artist.as_bytes());
        f.extend_from_slice(&colour.to_le_bytes());
        f
    }

    fn cat_frame(cats: &[(u8, u8, &str, u16)]) -> std::vec::Vec<u8> {
        let mut f = std::vec![b'M', b'1', CATEGORIES, cats.len() as u8];
        for (id, kind, name, colour) in cats {
            f.extend_from_slice(&[*id, *kind, name.len() as u8]);
            f.extend_from_slice(name.as_bytes());
            f.extend_from_slice(&colour.to_le_bytes());
        }
        f
    }

    fn item_bytes(title: &str, artist: &str, colour: u16, id: u8) -> std::vec::Vec<u8> {
        let mut f = std::vec![title.len() as u8];
        f.extend_from_slice(title.as_bytes());
        f.push(artist.len() as u8);
        f.extend_from_slice(artist.as_bytes());
        f.extend_from_slice(&colour.to_le_bytes());
        f.push(id);
        f
    }

    fn begin_frame(node: u8, kind: u8, total: u8, title: &str) -> std::vec::Vec<u8> {
        let mut f = std::vec![b'M', b'1', LIST_BEGIN, node, kind, total, title.len() as u8];
        f.extend_from_slice(title.as_bytes());
        f
    }

    fn items_frame(node: u8, start: u8, items: &[std::vec::Vec<u8>]) -> std::vec::Vec<u8> {
        let mut f = std::vec![b'M', b'1', ITEMS, node, start, items.len() as u8];
        for i in items {
            f.extend_from_slice(i);
        }
        f
    }

    fn root(m: &mut Model) {
        let f = cat_frame(&[
            (1, KIND_SONGS, "Quick picks", 0x1234),
            (2, KIND_SONGS, "Listen again", 0x2345),
            (3, KIND_FOLDER, "Downloads", 0x3456),
            (4, KIND_FOLDER, "Library", 0x4567),
            (5, KIND_FOLDER, "Explore", 0x5678),
        ]);
        assert_eq!(m.apply(&f, 10), Effect { root: true, ..Effect::default() });
    }

    #[test]
    fn root_categories_parse_and_keep_their_colours() {
        let mut m = Model::new();
        root(&mut m);
        assert_eq!(m.root_count, 5);
        assert_eq!(m.root[0].label(), "Quick picks");
        assert_eq!(m.root[0].colour, 0x1234);
        assert!(m.root[2].is_folder());
        assert!(!m.root[1].is_folder());
        assert!(m.connected);
        assert_eq!(m.last_rx, 10);
        // a shorter list replaces the old one entirely
        assert!(m.apply(&cat_frame(&[(9, KIND_SONGS, "Home", 0)]), 20).root);
        assert_eq!(m.root_count, 1);
        assert_eq!(m.root[1].label(), "");
    }

    #[test]
    fn a_list_streams_in_and_is_ready_on_the_end_frame() {
        let mut m = Model::new();
        root(&mut m);
        m.push(3, "Downloads");
        assert_eq!(m.list_node, 3);
        assert!(!m.list_ready);
        assert_eq!(m.apply(&begin_frame(3, KIND_SONGS, 3, "Downloaded songs"), 30).list, true);
        assert_eq!(m.list_title(), "Downloaded songs");
        let items = std::vec![
            item_bytes("First", "One", 0x1111, NONE),
            item_bytes("Second", "Two", 0x2222, NONE),
        ];
        assert!(m.apply(&items_frame(3, 0, &items), 31).list);
        assert_eq!(m.item_count, 2);
        assert_eq!(m.items[1].title(), "Second");
        assert_eq!(m.items[1].colour, 0x2222);
        assert!(!m.items[1].is_folder());
        let last = std::vec![item_bytes("Third", "Three", 0x3333, NONE)];
        assert!(m.apply(&items_frame(3, 2, &last), 32).list);
        assert!(!m.list_ready);
        assert_eq!(m.apply(b"M1E\x03\x03", 33), Effect { list: true, done: true, ..Effect::default() });
        assert!(m.list_ready);
        assert_eq!(m.item_count, 3);
    }

    #[test]
    fn folder_items_carry_a_node_and_back_walks_the_chain() {
        let mut m = Model::new();
        root(&mut m);
        m.push(4, "Library");
        assert_eq!(m.depth, 1);
        assert!(m.apply(&begin_frame(4, KIND_FOLDER, 2, "Library"), 40).list);
        let items = std::vec![
            item_bytes("Songs", "1,204", 0x0101, 20),
            item_bytes("Albums", "318", 0x0202, 21),
        ];
        m.apply(&items_frame(4, 0, &items), 41);
        m.apply(b"M1E\x04\x02", 42);
        assert!(m.items[0].is_folder());
        assert_eq!(m.items[0].id, 20);

        m.push(20, "Songs");
        assert_eq!(m.depth, 2);
        assert_eq!(m.list_node, 20);
        assert_eq!(m.pop(), Some(4));
        assert_eq!(m.list_node, 4);
        assert_eq!(m.depth, 1);
        assert_eq!(m.pop(), Some(0));
        assert_eq!(m.depth, 0);
        assert_eq!(m.pop(), None);
    }

    #[test]
    fn stale_and_malformed_frames_change_nothing() {
        let mut m = Model::new();
        root(&mut m);
        m.push(3, "Downloads");
        m.apply(&begin_frame(3, KIND_SONGS, 2, "Songs"), 50);
        let before = std::string::String::from(m.list_title());
        // another node's list is stale: we already left it
        assert_eq!(m.apply(&begin_frame(4, KIND_SONGS, 2, "Other"), 51), Effect::default());
        assert_eq!(m.list_title(), before.as_str());
        assert_eq!(m.apply(b"M1B\x03\x00\x02\x09Songs", 52), Effect::default()); // truncated title
        assert_eq!(m.apply(b"M1S\x08\x00\x32\x00", 53), Effect::default()); // unknown flag bit
        assert_eq!(m.apply(b"M1S\x00\x65\x32\x00\x00", 54), Effect::default()); // 101 percent
        assert_eq!(m.apply(b"M1C\x07", 55), Effect::default()); // too many categories
        assert_eq!(m.apply(b"M1I\x03\x13\x01\x00", 56), Effect::default()); // start beyond 20
        assert_eq!(m.apply(b"M1E\x03\x02\x00", 57), Effect::default()); // wrong length
        assert_eq!(m.apply(b"X1H", 58), Effect::default()); // wrong magic
        assert_eq!(m.apply(&[0xFF; MSG_MAX + 1], 59), Effect::default());
        assert!(!m.list_ready);
        assert_eq!(m.root_count, 5);
    }

    #[test]
    fn now_playing_and_volume_frames() {
        let mut m = Model::new();
        let f = now_frame(NOW_PLAYING | NOW_HAS_TRACK, 42, 70, "Song name", "Artist name", 0x1F3E);
        assert_eq!(m.apply(&f, 60), Effect { now: true, ..Effect::default() });
        assert!(m.now.has);
        assert!(m.now.playing);
        assert_eq!(m.now.pos, 42);
        assert_eq!(m.now.colour, 0x1F3E);
        assert_eq!(m.now.title(), "Song name");
        assert_eq!(m.now.artist(), "Artist name");
        let paused = now_frame(NOW_HAS_TRACK, 0, 70, "Song name", "Artist name", 0);
        assert!(m.apply(&paused, 61).now);
        assert!(!m.now.playing);
        // The colour is the last field: a frame that stops before it changes nothing.
        let mut short = now_frame(NOW_HAS_TRACK, 0, 70, "Song name", "Artist name", 0);
        short.truncate(short.len() - 2);
        assert_eq!(m.apply(&short, 62), Effect::default());
        assert_eq!(m.now.colour, 0);

        assert_eq!(Model::transport_frame(CMD_NEXT), *b"M1t\x01");
        assert_eq!(Model::play_frame(4, 7), *b"M1p\x04\x07");
        assert_eq!(Model::browse_frame(0), *b"M1b\x00");
        assert_eq!(Model::volume_frame(140), *b"M1v\x64");
        assert_eq!(Model::hello_frame(), *b"M1?");
        assert_eq!(Model::queue_frame(), *b"M1q");
        assert_eq!(Model::queue_play_frame(7), *b"M1j\x07");
    }

    #[test]
    fn the_queue_streams_in_and_marks_the_current_entry() {
        let mut m = Model::new();
        assert_eq!(m.apply(b"M1Q\x03\x01", 10), Effect { queue: true, ..Effect::default() });
        assert_eq!(m.queue_total, 3);
        assert_eq!(m.queue_current, 1);
        assert!(!m.queue_ready);
        let items = std::vec![
            item_bytes("First", "One", 0x1111, NONE),
            item_bytes("Second", "Two", 0x2222, NONE),
        ];
        // queue entries carry no node id: drop the trailing byte of each list item
        let mut q = std::vec![b'M', b'1', QUEUE_ITEMS, 0, 2];
        for i in &items {
            q.extend_from_slice(&i[..i.len() - 1]);
        }
        assert!(m.apply(&q, 11).queue);
        assert_eq!(m.queue_count, 2);
        assert_eq!(m.queue[1].title(), "Second");
        assert!(!m.queue_ready);
        let mut last = std::vec![b'M', b'1', QUEUE_ITEMS, 2, 1];
        let third = item_bytes("Third", "Three", 0x3333, NONE);
        last.extend_from_slice(&third[..third.len() - 1]);
        assert!(m.apply(&last, 12).queue);
        assert_eq!(m.queue_count, 3);
        assert!(m.queue_ready);
        // a malformed frame leaves the queue alone
        assert_eq!(m.apply(b"M1Q\x02\x05", 13), Effect::default());
        assert_eq!(m.apply(b"M1J\x00\x02\x09", 14), Effect::default());
        assert_eq!(m.queue_count, 3);
    }

    #[test]
    fn volume_steps_stay_in_range() {
        assert_eq!(Model::step_volume(50, 1), 55);
        assert_eq!(Model::step_volume(50, -1), 45);
        assert_eq!(Model::step_volume(98, 1), 100);
        assert_eq!(Model::step_volume(2, -1), 0);
        assert_eq!(Model::step_volume(0, -5), 0);
    }

    #[test]
    fn play_acks_arrive_and_bad_codes_change_nothing() {
        let mut m = Model::new();
        assert_eq!(m.apply(b"M1K\x00", 10), Effect { ack: true, ..Effect::default() });
        assert_eq!(m.ack, ACK_PLAYING);
        assert_eq!(m.apply(b"M1K\x01", 11), Effect { ack: true, ..Effect::default() });
        assert_eq!(m.ack, ACK_REFUSED);
        assert_eq!(m.apply(b"M1K\x02", 12), Effect { ack: true, ..Effect::default() });
        assert_eq!(m.ack, ACK_NO_SWITCH);
        // Unknown codes and wrong shapes are dropped, and the last good code stays.
        assert_eq!(m.apply(b"M1K\x03", 13), Effect::default());
        assert_eq!(m.apply(b"M1K", 14), Effect::default());
        assert_eq!(m.apply(b"M1K\x00\x00", 15), Effect::default());
        assert_eq!(m.ack, ACK_NO_SWITCH);
    }

    #[test]
    fn empty_and_refused_lists_are_done() {
        let mut m = Model::new();
        root(&mut m);
        m.push(5, "Explore");
        assert_eq!(m.apply(&begin_frame(5, KIND_SONGS, 0, "Explore"), 70), Effect { list: true, done: true, ..Effect::default() });
        assert!(m.list_ready);
        m.push(5, "Explore");
        assert_eq!(m.apply(b"M1X\x05\x01", 71), Effect { list: true, done: true, ..Effect::default() });
        assert_eq!(m.list_code, 1);
        assert!(m.list_ready);
        assert_eq!(m.item_count, 0);
        assert_eq!(m.apply(b"M1X\x05\x09", 72), Effect::default()); // unknown code
    }

    #[test]
    fn an_empty_queue_is_ready_at_once() {
        let mut m = Model::new();
        assert!(m.apply(b"M1Q\x00\x00", 10).queue);
        assert!(m.queue_ready);
        assert_eq!(m.queue_count, 0);
    }

    #[test]
    fn a_phone_silence_marks_the_link_offline() {
        let mut m = Model::new();
        root(&mut m);
        assert!(m.alive(20));
        assert!(!m.alive(26_000));
        m.mark_disconnected();
        assert!(!m.alive(20));
        assert_eq!(m.apply(b"M1H", 30), Effect { heartbeat: true, ..Effect::default() });
        assert!(m.alive(30));
    }

    #[test]
    fn two_items_fit_one_message() {
        let title = "x".repeat(TITLE_MAX);
        let artist = "y".repeat(ARTIST_MAX);
        let items = std::vec![
            item_bytes(&title, &artist, 0x1234, NONE),
            item_bytes(&title, &artist, 0x5678, 9),
        ];
        let frame = items_frame(1, 0, &items);
        assert!(frame.len() <= MSG_MAX, "frame is {} bytes", frame.len());
        let mut m = Model::new();
        m.open(1, "Songs");
        assert!(m.apply(&frame, 1).list);
        assert_eq!(m.item_count, 2);
    }
}
