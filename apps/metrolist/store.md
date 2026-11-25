# Metrolist

Your Metrolist library on the watch: the categories the phone actually has content for, a list of songs for each, and
a player that controls what the phone plays. Picking a song starts a queue from that song; the audio always plays on
the phone. Empty shelves such as Quick picks stay off the watch until Metrolist has something to show there.

Two things live in the app:

- **Controller** — the phone's current track and its artist above the play button (a title too long for the row
  scrolls round as a marquee), previous / play-pause / next around the centre, and a bottom row of Volume and Queue
  text buttons. The sleeve's dominant colour glows behind it (concentric bands: the watch's LVGL has no gradient), the
  phone sends that one 16-bit value with the track, and the progress ring takes it too. The controls are see-through,
  so the glow reads through the play button, the skips and both bottom buttons. The volume button opens its own screen: a capsule
  that follows the phone's volume, drag it or turn the crown, and the change goes to the phone with a Back pill below
  it. The Queue button opens the phone's queue (up to 20 songs, the playing one filled); tap an entry to jump to it
  in the same queue, and the Back pill at the bottom returns to the player.
- **Page** — long press its row in Apps → Add page and the player sits next to the watch face, a swipe away, with no
  browsing around it.
- **Metrolist** — the YouTube Music categories the phone's Metrolist app offers. Downloads and Library open their own
  sub-categories (songs, albums, playlists, artists); Back walks up one step.

Nothing personal crosses the watch's Bluetooth link: the account, the sign-in and the library stay in Metrolist on the
phone, and the watch only names a category, an item number and a play / pause / next command.

## Requirements

- [Metrolist](https://github.com/MetrolistGroup/Metrolist) with its watch link on (Settings → Watch), installed and
  signed in on the phone. The watch link needs the patched firmware with the channel key and an App Loader from
  2026-11-16 or later (the app owns Back, pages the controller sideways and draws with the stock LVGL widgets);
  nothing re-pairs and Nothing X keeps working.
- Settings → Watch also installs the bundled Metrolist watch package over that same link. Keep the watch awake during
  the upload; no separate installer or second pairing is used.
- The watch and the phone on the same Bluetooth link: one central at a time, so no second device (a Mac, say) while
  the phone is connected.
