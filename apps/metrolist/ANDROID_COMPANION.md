# Metrolist watch link — companion contract

This is the implementation contract for the Metrolist fork that pairs with `apps/metrolist`. The phone is the only
YouTube Music client: it owns the account and the sign-in, the downloads, the library and the player. The watch names
a node to browse and an item to play, and shows what the phone reports. The watch never stores an account token,
cookie, track URL or file.

The fork is [MetrolistGroup/Metrolist](https://github.com/MetrolistGroup/Metrolist) plus a **Settings → Watch** switch
and the module described here. Nothing else about the app changes. Metrolist is Kotlin Multiplatform: the BLE link is
Android-only, so it lives in `androidApp`; the node mapping is plain data and can live in `shared` if desktop and iOS
want it later.

## Connection model

1. The user pairs the watch once in Nothing X. The fork does not run a second pairing flow and must not perform the
   App Loader's bind handshake.
2. Resolve the already bonded watch and open the stub's channel with the pairing secret `K`, exactly as
   `.sdk/wp2-apps/docs/abi-1.4.md` ("Writing a host") describes: subscribe to `FFF1`, `77D4FF02-…` and
   `02F00000-…-FFE2`, write `AT GETSECRET` to the shell `77D4FF01-…`, then seal every frame with `K`. No bind, no
   "Paired" popup, Nothing X keeps working.
3. Treat `K` as a password: memory only, cleared when the GATT session closes, never logged or exported.
4. Register with one `PING` (`0x90A7`). App messages then arrive as `0xB0A6` events without polling; `send` writes
   go out as `MSG` (`0x90A5`) with slot `0xFF` (whatever app runs). An `EVENT` plaintext is **not** the app message:
   it is `seq: u8 | slot: u8 | data` (`abi-1.1.md`, "Data channel"), so strip the first two bytes before parsing `M1`.
5. Reconnect with backoff after BLE errors; Android needs a connected-device foreground service so the link survives
   the app being backgrounded. Re-send the full state after every reconnect.
6. If the watch never answers, show a compatibility note (patched firmware with the channel key, App Loader of
   2026-11-13 or later) instead of starting a bind.

`connectGatt` next to Nothing X is still untested on Android (AGENTS.md, "Apps that talk to a phone"). Exercise it on
each supported vendor/version before calling the feature shipped.

## Protocol (`M1`)

ASCII `M1`, one kind byte, then fixed fields. Every message is at most 128 bytes; the watch's inbox holds four, so the
phone sends one burst per answer and waits for the watch's next request. Integers are unsigned bytes unless noted.

### Watch → phone

| Frame | Meaning |
|---|---|
| `M1?` | hello: the app started, resumed, or lost the phone. Answer with `S`, then `C` (and the open node's list if the watch asks) |
| `M1b node` | browse `node`; `0` is the root categories |
| `M1p node index` | play item `index` of `node`: queue the node's songs from that item and start it |
| `M1t cmd` | transport: `0` play/pause, `1` next, `2` previous |
| `M1v level` | volume `0..=100` |
| `M1q` | the watch's queue screen opened (or came back to): answer with `Q` and `J` |
| `M1j index` | play queue entry `index` of the queue the phone has; the rest of the queue stays |
| `M1f len query` | search the library: answer with the best matches as list frames for `SEARCH_NODE` (`250`) |

The watch retries `?` every 0.6 s, doubling to 3 s, until the phone answers. A list request that is not answered in
3 s is sent again. Duplicate `p`/`t` frames are idempotent.

### Phone → watch

| Frame | Meaning |
|---|---|
| `M1S flags pos vol tlen title alen artist colour:u16le` | now playing. `flags`: bit 0 playing, bit 1 the phone's liked state (the watch ignores it), bit 2 a track is there. `pos` 0..=100 percent. `title` ≤ 34 bytes, `artist` ≤ 20. `colour` is the artwork's dominant RGB565, for the player's glow and ring; `0` when unknown |
| `M1C n (id kind nlen name colour:u16le)*` | the root categories, in order. `kind` 0 opens a folder, 1 is a song list. `name` ≤ 20 bytes, `colour` RGB565. `n` ≤ 6 |
| `M1B node kind total tlen title` | a list starts: `total` items follow, `title` ≤ 20 bytes |
| `M1I node start n (tlen title alen artist colour:u16le id)*` | items `start..start+n` of that list; `n` ≤ 20. A folder item's `id` is the node to browse next; a song's is `0xFF`. `colour` is the item's (or its artwork's average) RGB565 for the watch's row |
| `M1E node total` | the list is complete |
| `M1X node code` | no list: `0` empty, `1` error, `2` not ready (still loading, signed out) |
| `M1B` / `M1I` / `M1E` for node `250` | a search's results, in the same list frames as a node; the watch draws them and `M1p 250 index` plays from there |
| `M1Q total current` | the queue starts: `total` entries follow (≤ 20), `current` is the playing position |
| `M1J start n (tlen title alen artist colour:u16le)*` | queue entries `start..start+n`, same text caps as `I`, no node id; the queue is ready at `total` |
| `M1H` | heartbeat, about every 10 s; the watch shows "Waiting for phone" after 25 s of silence |

`node` ids are the phone's: stable while the process lives, `1..=200`, `0` = root. A list frame for a node the watch
has already left is dropped by the watch; the phone may send it anyway.

The watch keeps the first 20 items of a list. Two items per `I` frame keeps a frame under 128 bytes with the title and
artist caps above; send `start` + `n` so a lost frame can be resent.

The queue screen is the same: `Q` names the length and the playing position, `J` carries the entries two at a time,
and the watch fills the row at `current`. Answer `M1q` with the queue the player really has (`queueSongs` and
`queueState.currentIndex`), and answer `M1j` by moving the existing queue to that position, never by replacing it.
After a confirmed `M1j` send `Q`/`J` again so the filled row follows the new current entry.

## The root categories

Answer `M1b 0` with `M1C` built from the same data the app's own home screen uses, in this order and with these ids:

| id | name | kind | source |
|---|---|---|---|
| 1 | Quick picks | songs | the home screen's quick-picks shelf |
| 2 | Listen again | songs | the home screen's "Listen again" shelf |
| 3 | Downloads | folder | the local downloads database |
| 4 | Library | folder | the user's library |
| 5 | Explore | folder | the app's explore/moods entries |

Names are the app's localized strings, clipped to 20 UTF-8 bytes on a char boundary. Colours come from each shelf's
artwork (the app's dominant colour) so the watch's rows match the app.

## Search

The watch's keyboard sends the finished query as one `M1f len query` frame (at most 40 UTF-8 bytes, no NUL). The
phone runs its own search (`YouTubeRepository.search(query)`, which falls back to the local library when offline), keeps
the first five `SongItem`s and streams them as an ordinary list under node `250`: `M1B 250 1 5 "Results"`, then two `I`
frames for node `250`, then `M1E 250 5`. An empty result set is still `B`/`E` with `total 0`; a failed search answers
`M1X 250 1` ("could not load") and a still-loading or signed-out app `M1X 250 2`.

Node `250` is reserved for this and is never part of the browse tree. `M1p 250 index` plays that match and queues the
rest of the five behind it, in the order the watch showed them; the ack and the queue screen are the node's.

## Quick picks and album shelves

Quick picks and Listen again are the API shelf and the local history together, matched on the `FEmusic_quickpicks` /
`FEmusic_listen_again` browse id or on the shelf's title and merged by song id. The shelf is not always on the first page
of the home feed, so the link walks at most two continuation pages while it is missing; when the API has nothing (signed
out, offline) the local side fills in: `dao.quickPicks()`, then the most played songs of all time, then
`dao.forgottenFavorites()`, which is the same pool the app's own classic Quick picks samples. Listen again is the last
month of `mostPlayedSongs`. Quick picks is always a root category, even with no rows at all, so the option does not vanish
from the watch's menu; its node answers `M1X 1 2` when the phone has neither home data nor history. Each rebuild logs one
`WatchLink` line with the feed's section count and the shelf's row count, and dumps the feed's titles when the shelf is
empty.

A home shelf whose items are albums becomes a folder of albums: each row carries a node id, and that node answers with
the album's own tracks in album order (`YouTubeRepository.getAlbum(browseId)` → `AlbumPage.songs`). `M1p` on that node
plays the chosen song and queues the rest of the album behind it under the album's title — the same shape as a search's
results. Not every item type has a watch screen: playlists, artists and podcasts are skipped for now.

The root `C` frame carries up to eight categories. Its names share the space left after the fixed fields in order, each
taking what it needs and leaving one byte for every name after it, so eight shelves keep their full titles.

Every title and artist leaves this side through `WatchProtocol.clip`, which first reduces the text to printable ASCII
(`plain`): typographic quotes and dashes become ASCII, accents are folded away, emoji and CJK are dropped, spaces are
collapsed. The watch's stock fonts have no glyph outside ASCII, and a name cut inside a UTF-8 character was drawn as
nothing, which is what made some results look empty or wrong.

## Where it hooks in Metrolist

The pieces this needs already exist; the link is mostly an adapter:

| Need | Metrolist side |
|---|---|
| Home shelves | `HomeViewModel` and `YouTubeRepository.getHomePageData` → `HomePageData.sections` (`HomeSection.title`, `.items`, `.browseId`, `.params`; `shared/src/commonMain/kotlin/com/metrolist/shared/model/Models.kt`). Quick picks is also built locally in `HomeViewModel` |
| Downloads | `DownloadsScreen` over the `MusicDatabase` DAOs (`shared/.../db/daos/DatabaseDao.kt`): downloaded songs, albums, playlists |
| Library | `LibraryScreen` and the `LibraryFilter` enum (`SONGS`, `ARTISTS`, `ALBUMS`, `PLAYLISTS`, `PODCASTS`, `LIBRARY`) |
| Explore | `BrowseScreen`'s mood/genre entries (`BrowsePageData`) |
| Queue and play | `PlayerConnection.playQueueFromRemote(songs, startIndex, title)` (`androidApp/.../playback/PlayerConnection.kt`), which calls `MusicService.playQueueFromRemote` |
| Transport | `PlayerConnection.skipToNextFromRemote()`, `skipToPreviousFromRemote()`, `seekToFromRemote(ms)`, `currentSong: StateFlow<SongItem?>`, `isPlaying: StateFlow<Boolean>` |
| Like | the app's own library add/remove on the current `SongItem` (`libraryAddToken` / `libraryRemoveToken`) |
| Settings row | `SettingsScreen` (`androidApp/.../ui/screens/SettingsScreen.kt`), beside the existing device-connect entry |
| Row colours | `SongItem.thumbnail`; compute the dominant RGB565 once per item (weighted by saturation, so a white sleeve background does not win) and cache it |

`shared/.../deviceconnect/` is the app's existing peer-to-peer device link. It is LAN/HTTP with its own pairing, not
BLE: do not reuse its transport. Its playback bridge (`DeviceConnectPlayback`, `DeviceConnectRemote`) is the shape to
copy, and its settings sheet (`DeviceConnectSheet`) is the UI precedent for the Watch row.

The watch link must not touch `LoginScreen`/`YouTubeSession`; the account flow stays exactly as it is.

## Nodes

A node is either a **song list** (`kind 1`) or a **folder** (`kind 0`).

- A song list answers `M1B`/`M1I`/`M1E` with up to 20 songs. `M1p node index` queues that node's songs from `index`:
  the same order the watch was shown, so `index` means the same row on both sides. Build a fresh queue and start it
  through the app's normal player controller; do not shuffle unless the node itself is a shuffle.
- An album node is a song list with `fromAlbum`: the watch shows its tracks, and a tap queues the album from that row.
- A folder answers with folder items (each with a `node` id) and/or song items mixed in the same list, in the app's
  order. Tapping a folder item makes the watch send `M1b <id>`.
- Depth is the watch's business (it keeps four levels); the phone just answers the node it was asked for.

Suggested folder trees (the app's own sections, so the watch and the phone agree):

- **Downloads** (`kind 0`): Songs, Albums, Playlists. Each is a song list; Albums and Playlists may also be folders
  of their tracks, in which case give the album/playlist item a node id.
- **Library** (`kind 0`): Songs, Albums, Playlists, Artists. Artists is a folder of artist nodes; each artist node is a
  song list (or an album folder if the app has one).
- **Explore** (`kind 0`): the app's mood/genre entries; each is a song list.

An empty node answers `M1X node 0`. A signed-out or still-loading node answers `M1X node 2` so the watch can say
"Metrolist is not ready" instead of "Nothing here yet".

## Sign-in

Signing in to Google stays entirely in the fork's own settings (the existing Metrolist/InnerTube account flow: the
WebView sign-in and cookie import). The watch has no login surface and receives no credential, cookie or token; a
signed-out app answers `M1X node 2`. When the account changes, resend `S` and the root `C`.

## Playback and state

- `M1S` is authoritative. Send it on `?`, on every change of track, play state, volume or position bucket, and
  at least every 10 s (`M1H` alongside). `pos` is a whole percent; do not stream every second of progress. Drive it
  from `PlayerConnection.currentSong` and `isPlaying`, not from a timer that re-reads the UI.
- `M1t 0` toggles the app's current player; `1`/`2` map to `skipToNextFromRemote` /
  `skipToPreviousFromRemote`.
- `M1v` sets the media volume, not the watch's. Clamp to the phone's volume steps and report the result in `S`.
- Commands are serialized; a failure still answers with a fresh `M1S` so the watch never keeps an optimistic state.
- The app keeps playing when the watch app closes; the watch is only a remote.

## Settings → Watch

One switch ("Connect to watch"), the connection state (Bluetooth, channel, last message), and a **Test connection**
action. When off, the fork must not hold the GATT link. Never show `K`, raw frames or track URLs in logs or
diagnostics.

## Privacy

Only counts, names the user already sees, colours and play state cross the link; anyone in BLE range can read the
channel. Never send the account email, cookies, tokens, file paths or download URLs. Keep `K` in memory.

## Release gates

- Firmware: channel key `K` needs the patched stub and a loader from 2026-11-13 or later; `own_back`, the LVGL app and
  its pages need the loader the app names in `store.md`: **2026-11-27**, `loader-2026-11-27-2018e1a.wp2app`, built from
  cmf-watch-sdk `2018e1a` and still listed under Experimental in FMC.
- Android: Nothing X + the fork at the same time on each supported vendor/version (no new pairing popup, both keep
  notifications, the channel survives screen-off and a service restart).
- Watch: `make test APP=metrolist`, `make build APP=metrolist`, `make check`, then the emulator or a real watch
  (owner's yes) for the screens, the crown volume and Back.
