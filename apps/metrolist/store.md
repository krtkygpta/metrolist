# Metrolist

Your Metrolist library on the watch: the categories the phone actually has content for, a list of songs for each, and
a player that controls what the phone plays. Picking a song starts a queue from that song; the audio always plays on
the phone. Empty shelves such as Quick picks stay off the watch until Metrolist has something to show there.

The phone half is the open-source Metrolist app with the watch link added: download the APK from
<https://github.com/krtkygpta/metrolist/releases/latest>, no Play Store and no account beyond your own YouTube Music.

What lives in the app:

- **Controller** — the phone's current track and its artist above the play button (a title too long for the row
  scrolls round as a marquee), previous / play-pause / next around the centre, and a bottom row with one icon each for
  Volume and Queue. The sleeve's dominant colour glows behind it (concentric bands: the watch's LVGL has no gradient),
  the phone sends that one 16-bit value with the track, and the progress ring takes it too. The controls carry Google's
  Material Icons — play, pause, skip back, skip forward, volume and queue — drawn from bitmaps in the package, each on
  a black control of its own so the glow reads around it. The volume button opens its own screen: a tall white vertical
  slider, or the crown, steps the phone's own media volume through the watch's media keys, and the Back arrow sits in
  the top-left corner. The Queue button opens the phone's queue (up to 20 songs, the playing one filled); tap an entry to jump to
  it in the same queue, and the Back arrow in the top-left corner returns to the player.
- **Start screen** — Search, Metrolist and Controller, a stock focus list that scrolls round the screen with the crown
  or a drag, the row in the middle of the face carrying the only plate; the arrow in the top-left corner leaves the app.
- **Widget** — long press its row in Apps → Add widget and a card left of the watch face shows the track the phone
  plays; a tap on the card opens the app. The card is written only when the track changes.
- **Page** — long press its row in Apps → Add page and the player sits next to the watch face, a swipe away, with no
  browsing around it.
- **Metrolist** — the YouTube Music categories the phone's Metrolist app offers: Quick picks (the app's own shelf), Listen
  again, Downloads, Library, Explore and the home shelves, including the ones that are shelves of albums. An album opens
  its own track list, and a song tapped there plays the album from that song with the rest queued behind it. Downloads
  and Library open their own sub-categories; Back walks up one step.
- **Search** — a keyboard on the watch (its own QWERTY grid, not the phone's, filling the bottom of the screen): type
  a name, tap Search, and the phone answers with its five best matches. Choose one and it plays, with the rest of the
  matches queued behind it; the Player and Queue screens are the same ones a browse tap leads to. Names arrive as
  printable ASCII — typographic quotes and dashes become plain ones, accents are folded away and emoji are dropped —
  because the watch's stock fonts have no glyph for anything else.

Nothing personal crosses the watch's Bluetooth link: the account, the sign-in and the library stay in Metrolist on the
phone, and the watch only names a category, an item number, a search word and a play / pause / next command.

## Requirements

- **[Metrolist with the watch link](https://github.com/krtkygpta/metrolist/releases/latest)** on the phone: download
  the APK (the release page also has the source, the `.wp2app` and the build instructions), install it next to your
  existing Metrolist — the debug build is `com.metrolist.music.debug`, so nothing is overwritten — then open
  **Settings → Watch** and turn the watch link on. Sign in to YouTube Music there; the watch has no account of its own.
- FMC patched firmware with the channel key and **the App Loader of 2026-11-27** — `loader-2026-11-27-2018e1a.wp2app`,
  built from cmf-watch-sdk `2018e1a`. FMC still lists that loader under **Experimental**; install it from there. This app
  needs exactly that one or newer: it owns Back, pages the controller sideways, draws with the stock LVGL widgets and
  shows its icons from bitmap assets in the package, and an older loader ignores its own Back and the sideways paging.
  Nothing re-pairs and Nothing X keeps working.
- Settings → Watch also installs the bundled Metrolist watch package over that same link: **Install Metrolist on
  watch**. Keep the watch awake during the upload; no separate installer or second pairing is used.
- The watch and the phone on the same Bluetooth link: one central at a time, so no second device (a Mac, say) while
  the phone is connected.
