//! Metrolist Remote: browse and play your Metrolist (YouTube Music) library from the watch, and control the phone's
//! playback. The watch never plays audio and never sees the account: it names a node to browse and an item to play,
//! and the Metrolist app on the phone does the rest over the app channel (`ANDROID_COMPANION.md`).
//!
//! Screens: the start screen (Controller, Metrolist), the player, the phone's queue, the root categories and one
//! node's folders or songs. The crown changes the volume after a tap on the volume button. Back is the app's own
//! (`own_back`): it walks the browse chain and leaves from the start screen; from the queue it returns to the player.
//! The app is also a watch page (`app_features!(PAGE)`): there it shows the player alone, and the watch keeps the
//! keys.
#![cfg_attr(any(target_os = "none", wp2_sim), no_main, cfg_attr(target_os = "none", no_std))]

#[cfg_attr(not(any(target_os = "none", wp2_sim)), allow(dead_code))]
mod model;
#[cfg_attr(not(any(target_os = "none", wp2_sim)), allow(dead_code))]
mod screens;

#[cfg(not(any(target_os = "none", wp2_sim)))]
fn main() {}

#[cfg(any(target_os = "none", wp2_sim))]
mod device {
    use crate::model::*;
    use crate::screens::{self, Kind, Screen};
    use cmf_wp2_app::lv::{Batch, Lv, ROOT};
    use cmf_wp2_app::{Api, BUTTON, CROWN, LV, MESSAGE, MSG_MAX, PAUSE, RESUME};

    cmf_wp2_app::entry!(main);
    cmf_wp2_app::app_icon!();
    // A menu app and a page next to the watch face (the player alone).
    cmf_wp2_app::app_features!(cmf_wp2_app::PAGE);

    /// The hello goes out again this soon while the phone has not answered; each try waits twice as long.
    const HELLO_MS: u32 = 600;
    const HELLO_MAX_MS: u32 = 3000;
    /// How long a browse may take before the watch asks again.
    const LIST_MS: u32 = 3000;
    /// How long the phone has to ack a play command before the watch says "no answer".
    const PLAY_ACK_MS: u32 = 10_000;
    /// Crown turns go to the phone at most this often (the last value is always sent).
    const VOL_GAP_MS: u32 = 120;
    /// A volume echo from the phone is ignored this long after the crown set it.
    const VOL_LOCAL_MS: u32 = 1500;
    /// How often the loop wakes when nothing is due (the clock, a late frame).
    const IDLE_MS: u32 = 5000;
    /// The marquee: 3 px every 60 ms, and a pause at each end so the start of the title can be read.
    const ROLL_MS: u32 = 60;
    const ROLL_PX: i16 = 3;
    const ROLL_HOLD_MS: u32 = 1400;
    /// A second back within this long is the same swipe delivered twice, not a second press.
    const BACK_GAP_MS: u32 = 250;

    static mut MODEL: Model = Model::new();

    fn after(now: u32, t: u32) -> bool {
        (now.wrapping_sub(t) as i32) >= 0
    }

    struct App {
        api: &'static Api,
        lv: Lv,
        b: Batch,
        s: Screen,
        m: &'static mut Model,
        /// The phone answered the last hello.
        heard: bool,
        tries: u8,
        next_hello: u32,
        list_deadline: u32,
        /// The play ack deadline while the watch shows "Playing...".
        play_pending: Option<u32>,
        /// The category version the browse page was built from.
        root_seen: u8,
        vol_dirty: bool,
        vol_sent_at: u32,
        vol_local_at: u32,
        last_min: u16,
        last_vol: u8,
        /// The artwork colour the player's glow was drawn from: a colour the phone repeats is not written again.
        last_colour: u16,
        /// The title the label was last given. Writing it restarts the marquee, and the phone repeats the same title
        /// with every progress frame, so it is written only when it really changed.
        title_seen: [u8; TITLE_MAX],
        title_len: u8,
        /// The marquee: how far the title has slid, when it steps next, and whether it is resting at the start.
        roll_x: i16,
        roll_at: u32,
        roll_hold: bool,
        /// The `S` version the screen was last drawn from.
        last_now: u8,
        /// Back from the controller returns to the song list the song was tapped in, not to the start screen.
        back_to_list: bool,
        /// When the last back was handled, to drop a doubled `BUTTON` from one swipe.
        last_back: u32,
        /// The watch started the app as its page: the player alone, and the watch keeps the keys.
        on_page: bool,
        /// The `queue_version` the screen was last drawn from.
        queue_seen: u8,
    }

    impl App {
        fn now(&self) -> u32 {
            self.api.now()
        }

        fn send(&self, data: &[u8]) {
            self.api.send(data);
        }

        fn clock(&self, buf: &mut [u8; 5]) -> u16 {
            match self.api.time() {
                Some(t) => {
                    screens::clock_text(t.hour, t.minute, buf);
                    t.hour as u16 * 60 + t.minute as u16
                }
                None => {
                    *buf = *b"--:--";
                    u16::MAX
                }
            }
        }

        /// Replaces the screen by the one of `kind`, built from the model.
        fn go(&mut self, kind: Kind) {
            let mut clock = [0u8; 5];
            let min = self.clock(&mut clock);
            let text = core::str::from_utf8(&clock).unwrap_or("");
            let App { b, lv, s, m, .. } = self;
            let mut flush = |b: &mut Batch| b.submit(lv);
            if screens::leave(b, s).is_ok() {
                // The page is gone even if the next build fails: never keep a handle that may be reused.
                s.page = ROOT;
                s.title = ROOT;
                s.status = ROOT;
                s.hint = ROOT;
                s.back_btn = ROOT;
                s.queue_btn = ROOT;
                s.queue_count = 0;
                s.volume_slider = ROOT;
                s.volume_text = ROOT;
            }
            let built = match kind {
                Kind::Home => screens::home(b, &mut flush),
                Kind::Controller => screens::controller(b, &mut flush, m, text),
                Kind::Queue => screens::queue(b, &mut flush, m),
                Kind::Volume => screens::volume(b, &mut flush, m),
                Kind::Browse => screens::browse(b, &mut flush, m),
                Kind::List => screens::list(b, &mut flush, m),
            };
            if let Ok(next) = built {
                *s = next;
                let _ = b.submit(lv);
            }
            self.last_min = min;
            self.last_now = self.m.now_version;
            self.last_vol = self.m.now.vol;
            self.last_colour = self.m.now.colour;
            self.root_seen = self.m.root_version;
            // The build wrote the title: remember it so the first refresh does not restart a marquee.
            let n = (self.m.now.tlen as usize).min(TITLE_MAX);
            self.title_seen = [0; TITLE_MAX];
            self.title_seen[..n].copy_from_slice(&self.m.now.title[..n]);
            self.title_len = n as u8;
            self.roll_reset();
        }

        /// The labels the phone can change without a rebuild.
        fn refresh(&mut self) {
            if self.s.kind == Kind::Volume {
                let s = self.s;
                let _ = screens::show_vol(&mut self.b, &s, self.m.now.vol);
                let _ = self.b.submit(&self.lv);
                return;
            }
            if self.s.kind != Kind::Controller {
                return;
            }
            let alive = self.m.alive(self.now());
            let mut clock = [0u8; 5];
            self.clock(&mut clock);
            let text = core::str::from_utf8(&clock).unwrap_or("");
            let s = self.s;
            let m = &*self.m;
            let n = (m.now.tlen as usize).min(TITLE_MAX);
            let new_title = n != self.title_len as usize || self.title_seen[..n] != m.now.title[..n];
            let mut flush = |b: &mut Batch| b.submit(&self.lv);
            let _ = screens::show_now(&mut self.b, &mut flush, &s, m, true);
            let _ = screens::show_clock(&mut self.b, &s, text);
            let artist = if alive { m.now.artist() } else { "Waiting for phone" };
            let _ = self.b.text(s.artist, artist);
            if new_title {
                let _ = screens::show_track(&mut self.b, &s, m.now.title());
            }
            let _ = self.b.submit(&self.lv);
            self.last_now = m.now_version;
            self.last_vol = m.now.vol;
            if new_title {
                self.title_seen[..n].copy_from_slice(&m.now.title[..n]);
                self.title_len = n as u8;
                self.roll_reset();
            }
        }

        // ---- the marquee ----

        /// Only the player's title can scroll, and only when it is wider than its box.
        fn rolling(&self) -> bool {
            self.s.kind == Kind::Controller && screens::title_rolls(self.m.now.title())
        }

        /// How far the title has to slide for its end to come into the box.
        fn roll_span(&self) -> i16 {
            (screens::title_width(self.m.now.title()) - screens::TITLE_BOX).clamp(0, i16::MAX as i32) as i16
        }

        /// Back to the beginning, with the pause that lets it be read.
        fn roll_reset(&mut self) {
            self.roll_x = 0;
            self.roll_hold = true;
            self.roll_at = self.now().wrapping_add(ROLL_HOLD_MS);
        }

        /// One step: rest at the start, slide left a little, then start over once the whole title has gone by.
        fn roll_tick(&mut self) {
            if !self.rolling() {
                return;
            }
            let now = self.now();
            if !after(now, self.roll_at) {
                return;
            }
            if self.roll_hold {
                self.roll_hold = false;
            } else {
                self.roll_x = self.roll_x.saturating_add(ROLL_PX);
                if self.roll_x >= self.roll_span() {
                    self.roll_x = 0;
                    self.roll_hold = true;
                }
            }
            let s = self.s;
            let _ = screens::roll_to(&mut self.b, &s, self.roll_x);
            let _ = self.b.submit(&self.lv);
            self.roll_at = now.wrapping_add(if self.roll_hold { ROLL_HOLD_MS } else { ROLL_MS });
        }

        // ---- the phone ----

        fn hello(&mut self) {
            self.send(&Model::hello_frame());
            self.heard = false;
            let gap = (HELLO_MS << self.tries.min(3)).min(HELLO_MAX_MS);
            self.tries = self.tries.saturating_add(1);
            self.next_hello = self.now().wrapping_add(gap);
        }

        fn hello_now(&mut self) {
            self.tries = 0;
            self.hello();
        }

        fn needs_phone(&self) -> bool {
            self.s.kind != Kind::Home
        }

        /// The queue is a screen of its own, with a Back pill: the phone's queue, asked for when it opens.
        fn enter_queue(&mut self) {
            self.go(Kind::Queue);
            self.ask_queue();
        }

        /// Opens the player. `from_list` keeps the song list as the way back, which is where a tap on a song was.
        fn enter_controller(&mut self, from_list: bool) {
            self.back_to_list = from_list && self.m.list_ready;
            self.go(Kind::Controller);
            self.hello_now();
        }

        /// Ask the phone for the queue; the screen is drawn again when it is complete (`queue_ready`).
        fn ask_queue(&mut self) {
            self.send(&Model::queue_frame());
        }

        fn enter_browse(&mut self) {
            self.back_to_list = false;
            self.go(Kind::Browse);
            self.hello_now();
            let frame = Model::browse_frame(0);
            self.send(&frame);
        }

        /// Opens `node` as a list and asks the phone for it. `push` remembers the way back.
        fn open_list(&mut self, node: u8, title: &str, push: bool) {
            if push {
                self.m.push(node, title);
            } else {
                self.m.open(node, title);
            }
            self.go(Kind::List);
            let frame = Model::browse_frame(node);
            self.send(&frame);
            self.list_deadline = self.now().wrapping_add(LIST_MS);
            self.play_pending = None;
        }

        fn phone(&mut self, msg: &[u8]) {
            let now = self.now();
            let was_connected = self.m.connected;
            let fx = self.m.apply(msg, now);
            if fx == Effect::default() {
                return;
            }
            self.heard = true;
            if fx.ack {
                // The play round trip finished: show what the phone did with the tap.
                self.play_pending = None;
                let text = match self.m.ack {
                    ACK_PLAYING => "Playing now",
                    ACK_REFUSED => "Refused",
                    _ => "Not switched",
                };
                let s = self.s;
                let _ = screens::show_status(&mut self.b, &s, text);
                let _ = self.b.submit(&self.lv);
                if self.m.ack == ACK_PLAYING {
                    if self.s.kind == Kind::Controller {
                        self.refresh();
                    } else if self.s.kind != Kind::Queue {
                        // A tap in the queue stays there: the phone sends the queue again with the new entry.
                        self.enter_controller(true);
                    }
                }
            }
            // Back from a phone restart or a reconnect: what the screen shows may be stale, ask again.
            if !was_connected && self.m.connected {
                match self.s.kind {
                    Kind::Browse => {
                        let frame = Model::browse_frame(0);
                        self.send(&frame);
                    }
                    Kind::List => {
                        self.m.list_ready = false;
                        self.list_deadline = now.wrapping_add(LIST_MS);
                        let frame = Model::browse_frame(self.m.list_node);
                        self.send(&frame);
                    }
                    Kind::Queue => self.ask_queue(),
                    _ => {}
                }
            }
            // The crown is ahead of the phone's echo of our own volume change.
            if fx.now && now.wrapping_sub(self.vol_local_at) < VOL_LOCAL_MS {
                self.m.now.vol = self.last_vol;
            }
            if fx.now && matches!(self.s.kind, Kind::Controller | Kind::Volume) {
                if self.s.kind == Kind::Controller && self.m.now.colour != self.last_colour {
                    // The glow's arcs are cut where the colour really steps in 16 bits, so a colour that lands later
                    // (the phone reads the cover after the track) needs the player drawn again.
                    self.go(Kind::Controller);
                } else if self.m.now_version != self.last_now {
                    self.refresh();
                }
            }
            // The queue streams in frames: draw the screen once, when the phone has sent it all.
            if fx.queue
                && self.s.kind == Kind::Queue
                && self.m.queue_ready
                && self.m.queue_version != self.queue_seen
            {
                self.queue_seen = self.m.queue_version;
                self.go(Kind::Queue);
            }
            if fx.root && self.s.kind == Kind::Browse && self.m.root_version != self.root_seen {
                self.go(Kind::Browse);
            }
            if fx.list && self.s.kind == Kind::List {
                if fx.done {
                    self.go(Kind::List);
                } else if self.s.title != ROOT {
                    let s = self.s;
                    let _ = screens::show_title(&mut self.b, &s, self.m);
                    let _ = self.b.submit(&self.lv);
                }
            }
        }

        // ---- input ----

        /// True for the first `BUTTON` of a press: one swipe must never walk two screens.
        fn back_pressed(&mut self) -> bool {
            let now = self.now();
            if now.wrapping_sub(self.last_back) < BACK_GAP_MS {
                return false;
            }
            self.last_back = now;
            true
        }

        /// Back: up one node, to the start screen, or out of the app (true).
        fn back(&mut self) -> bool {
            match self.s.kind {
                Kind::Home => true,
                Kind::Controller => {
                    // A song tapped in a list returns there; the start screen is only the way back from its row.
                    // The watch's page has no start screen, so back there stays on the player.
                    if self.back_to_list {
                        self.go(Kind::List);
                    } else if !self.on_page {
                        self.go(Kind::Home);
                    }
                    false
                }
                Kind::Volume => {
                    self.go(Kind::Controller);
                    false
                }
                Kind::Queue => {
                    self.go(Kind::Controller);
                    false
                }
                Kind::Browse => {
                    self.go(Kind::Home);
                    false
                }
                Kind::List => {
                    match self.m.pop() {
                        Some(0) => {
                            self.go(Kind::Browse);
                            let frame = Model::browse_frame(0);
                            self.send(&frame);
                        }
                        Some(node) => {
                            self.go(Kind::List);
                            let frame = Model::browse_frame(node);
                            self.send(&frame);
                            self.list_deadline = self.now().wrapping_add(LIST_MS);
                        }
                        None => self.go(Kind::Home),
                    }
                    false
                }
            }
        }

        fn tap(&mut self, handle: u16, code: i16) {
            if code != screens::CLICKED {
                return;
            }
            match self.s.kind {
                Kind::Home => match screens::row_at(&self.s, handle) {
                    Some(0) => self.enter_controller(false),
                    Some(1) => self.enter_browse(),
                    _ => {}
                },
                Kind::Browse => {
                    let Some(i) = screens::row_at(&self.s, handle) else { return };
                    if i >= self.m.root_count as usize {
                        return;
                    }
                    let cat = self.m.root[i];
                    if cat.id == NONE {
                        return;
                    }
                    let mut name = [0u8; NAME_MAX];
                    let len = (cat.nlen as usize).min(NAME_MAX);
                    name[..len].copy_from_slice(&cat.name[..len]);
                    let title = core::str::from_utf8(&name[..len]).unwrap_or(ROOT_TITLE);
                    self.open_list(cat.id, title, true);
                }
                Kind::List => {
                    let Some(i) = screens::row_at(&self.s, handle) else { return };
                    if i >= self.m.item_count as usize {
                        return;
                    }
                    let item = self.m.items[i];
                    if item.is_folder() {
                        let mut name = [0u8; TITLE_MAX];
                        let len = (item.tlen as usize).min(TITLE_MAX);
                        name[..len].copy_from_slice(&item.title[..len]);
                        let title = core::str::from_utf8(&name[..len]).unwrap_or("Folder");
                        self.open_list(item.id, title, true);
                    } else {
                        let frame = Model::play_frame(self.m.list_node, i as u8);
                        self.send(&frame);
                        // Stay on the list and show the round trip: the phone acks once the song really plays.
                        self.play_pending = Some(self.now().wrapping_add(PLAY_ACK_MS));
                        let s = self.s;
                        let _ = screens::show_status(&mut self.b, &s, "Playing...");
                        let _ = self.b.submit(&self.lv);
                    }
                }
                Kind::Controller => {
                    if handle == self.s.prev.0 {
                        self.send(&Model::transport_frame(CMD_PREV));
                    } else if handle == self.s.play.0 {
                        self.send(&Model::transport_frame(CMD_TOGGLE));
                    } else if handle == self.s.next.0 {
                        self.send(&Model::transport_frame(CMD_NEXT));
                    } else if handle == self.s.vol.0 {
                        // The volume is its own screen: a capsule that follows and sets the phone's volume.
                        self.go(Kind::Volume);
                    } else if handle == self.s.queue_btn.0 {
                        self.enter_queue();
                    }
                }
                Kind::Queue => {
                    // The Back pill only goes back; the rows play that queue position in the same queue.
                    if handle == self.s.back_btn.0 {
                        self.go(Kind::Controller);
                        return;
                    }
                    let Some(i) = screens::queue_at(&self.s, handle) else { return };
                    self.m.queue_current = i as u8;
                    self.send(&Model::queue_play_frame(i as u8));
                    let s = self.s;
                    let _ = screens::show_queue(&mut self.b, &s, self.m);
                    let _ = self.b.submit(&self.lv);
                }
                Kind::Volume => {
                    // Only the Back pill leaves the volume screen; the capsule and the crown do the rest.
                    if handle == self.s.back_btn.0 {
                        self.go(Kind::Controller);
                    }
                }
            }
        }

        // ---- the volume on the crown ----

        fn vol_flush(&mut self, force: bool) {
            if !self.vol_dirty {
                return;
            }
            let now = self.now();
            if force || now.wrapping_sub(self.vol_sent_at) >= VOL_GAP_MS {
                self.send(&Model::volume_frame(self.m.now.vol));
                self.vol_sent_at = now;
                self.vol_dirty = false;
            }
        }

        /// The crown changes the phone's volume on the player and the volume screen; no arming tap needed.
        fn crown(&mut self, delta: i16) {
            if delta == 0 || !matches!(self.s.kind, Kind::Controller | Kind::Volume) {
                return;
            }
            self.vol_local_at = self.now();
            let level = Model::step_volume(self.m.now.vol, delta);
            if level != self.m.now.vol {
                self.m.now.vol = level;
                self.last_vol = level;
                self.vol_dirty = true;
                self.vol_flush(false);
                if self.s.kind == Kind::Volume {
                    let s = self.s;
                    let _ = screens::show_vol(&mut self.b, &s, level);
                    let _ = self.b.submit(&self.lv);
                }
            }
        }

        /// The volume screen's capsule, mid-drag: show it at once and send when it is let go.
        fn volume_drag(&mut self, value: i16) {
            let level = value.clamp(0, 100) as u8;
            if level == self.m.now.vol {
                return;
            }
            self.m.now.vol = level;
            self.last_vol = level;
            self.vol_local_at = self.now();
            self.vol_dirty = true;
            let s = self.s;
            let _ = screens::show_vol(&mut self.b, &s, level);
            let _ = self.b.submit(&self.lv);
        }

        fn vol_tick(&mut self) {
            self.vol_flush(false);
        }

        // ---- timers ----

        fn tick(&mut self) {
            let now = self.now();
            if self.m.connected && !self.m.alive(now) {
                self.m.mark_disconnected();
                self.heard = false;
            }
            if !self.heard && self.needs_phone() && after(now, self.next_hello) {
                self.hello();
            }
            if self.s.kind == Kind::List && !self.m.list_ready && after(now, self.list_deadline) {
                let frame = Model::browse_frame(self.m.list_node);
                self.send(&frame);
                self.list_deadline = now.wrapping_add(LIST_MS);
            }
            if let Some(deadline) = self.play_pending {
                if after(now, deadline) {
                    self.play_pending = None;
                    let s = self.s;
                    let _ = screens::show_status(&mut self.b, &s, "No answer");
                    let _ = self.b.submit(&self.lv);
                }
            }
            if self.s.kind == Kind::Controller {
                let mut buf = [0u8; 5];
                let min = self.clock(&mut buf);
                if min != self.last_min {
                    let text = core::str::from_utf8(&buf).unwrap_or("");
                    let _ = screens::show_clock(&mut self.b, &self.s, text);
                    let _ = self.b.submit(&self.lv);
                    self.last_min = min;
                }
            }
            self.roll_tick();
            self.vol_tick();
        }

        fn wait(&self) -> u32 {
            let now = self.now();
            let mut w = IDLE_MS;
            if !self.heard && self.needs_phone() {
                w = w.min((self.next_hello.wrapping_sub(now) as i32).max(0) as u32);
            }
            if self.s.kind == Kind::List && !self.m.list_ready {
                w = w.min((self.list_deadline.wrapping_sub(now) as i32).max(0) as u32);
            }
            if self.play_pending.is_some() {
                w = w.min(1000);
            }
            if self.vol_dirty {
                w = w.min(VOL_GAP_MS.saturating_sub(now.wrapping_sub(self.vol_sent_at)));
            }
            if self.rolling() {
                w = w.min((self.roll_at.wrapping_sub(now) as i32).max(0) as u32);
            }
            w
        }
    }

    /// True on RESUME, false on BUTTON. A page never exits itself, so there a BUTTON keeps waiting.
    fn paused(api: &Api, page: bool) -> bool {
        loop {
            match api.event(u32::MAX).map(|e| e.kind) {
                Some(RESUME) => return true,
                Some(BUTTON) if !page => return false,
                _ => {}
            }
        }
    }

    fn main(api: &'static Api) -> i32 {
        let Some(lv) = api.lv() else { return -95 };
        let Some(mut b) = Batch::new() else { return -1 };
        if screens::fonts(&mut b).and_then(|_| b.submit(&lv)).is_err() {
            return 1;
        }
        // First in the first screen batch: the system font and white text for every label.
        if screens::theme(&mut b).is_err() {
            return 2;
        }
        // Back walks the browse chain and leaves from the start screen: the app owns it.
        api.own_back(true);
        let m = unsafe { &mut *core::ptr::addr_of_mut!(MODEL) };
        let mut app = App {
            api,
            lv,
            b,
            s: Screen::empty(),
            m,
            heard: false,
            tries: 0,
            next_hello: 0,
            list_deadline: 0,
            play_pending: None,
            root_seen: 0,
            vol_dirty: false,
            vol_sent_at: 0,
            vol_local_at: 0,
            last_min: u16::MAX,
            last_vol: 0,
            last_colour: 0,
            title_seen: [0; TITLE_MAX],
            title_len: 0,
            roll_x: 0,
            roll_at: 0,
            roll_hold: true,
            last_now: 0,
            back_to_list: false,
            last_back: 0,
            on_page: api.on_page(),
            queue_seen: 0,
        };
        if app.on_page {
            // The watch's page next to the face: the player alone, no way out (the watch keeps the keys).
            app.enter_controller(false);
        } else {
            app.go(Kind::Home);
        }

        let mut buf = [0u8; MSG_MAX];
        let mut got = [0u8; MSG_MAX];
        loop {
            app.tick();
            let wait = app.wait();
            match api.event(wait) {
                Some(e) if e.kind == BUTTON => {
                    // Back always walks the app's own screens; only a menu app may leave (a page never exits itself).
                    if app.back_pressed() && app.back() && !app.on_page {
                        return 0;
                    }
                }
                Some(e) if e.kind == CROWN => app.crown(e.delta),
                Some(e) if e.kind == MESSAGE => {
                    while let Some(msg) = api.recv(&mut buf) {
                        let n = msg.len();
                        got[..n].copy_from_slice(msg);
                        app.phone(&got[..n]);
                    }
                }
                Some(e) if e.kind == PAUSE => {
                    // Going home from a page (even from the queue) only pauses it: exiting from a page crashed the
                    // watch, so a page app waits for its RESUME instead of returning.
                    if !paused(api, app.on_page) {
                        return 0;
                    }
                    app.hello_now();
                    app.refresh();
                }
                Some(e) if e.kind == LV => {
                    if e.y == screens::VALUE_CHANGED && e.x as u16 == app.s.volume_slider.0 {
                        app.volume_drag(e.delta);
                    } else if e.y == screens::RELEASED && e.x as u16 == app.s.volume_slider.0 {
                        app.vol_flush(true);
                    } else {
                        app.tap(e.x as u16, e.y);
                    }
                }
                _ => {}
            }
        }
    }
}
