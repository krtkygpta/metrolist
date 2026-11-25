# Metrolist watch link for the CMF Watch Pro 2

Your Metrolist (YouTube Music) library on the watch, and a player that controls what the phone plays. The watch shows
the categories the phone's Metrolist app actually has content for, a list of songs for each, and a player with
play/pause, previous/next, the phone's volume and its queue. Picking a song starts a queue from that song; the audio
always plays on the phone.

- `apps/metrolist/` — the watch app, the same files as the [`metrolist-companion` branch of the FMC apps
  repository](https://github.com/fmctools/apps), which is where it can be merged into the store. `store.toml` and
  `store.md` are its listing, and `ANDROID_COMPANION.md` is the watch-phone protocol contract.
- `android/` — the Android companion: [MetrolistGroup/Metrolist](https://github.com/MetrolistGroup/Metrolist) plus the
  watch link (Settings → Watch). It owns the account, the downloads, the library and the player; the watch only names a
  category, an item and a command.

Nothing personal crosses the Bluetooth link: no account, sign-in, cookie, token, track URL or file path. Only titles,
artists, 16-bit row colours and play state. Anyone in BLE range can read the channel, so it is built that way.

## Installing the watch app from a PC

A watch with the [FMC patched firmware and App Loader](https://fmc.freethinkel.dev/firmware) is required: the channel
key K needs a stub and loader from 2026-11-13 or newer, and `own_back` / the player page need a loader from
2026-11-16 or newer.

Toolchain: [Rust](https://rustup.rs) with `rustup target add thumbv7em-none-eabi` and
`rustup component add llvm-tools`, [uv](https://docs.astral.sh/uv/), [resvg](https://github.com/linebender/resvg),
`make`, and a C linker.

```bash
git clone https://github.com/<you>/metrolist
cd metrolist
make sdk                       # fetch cmf-watch-sdk at the pinned commit into .sdk/wp2-apps
make test APP=metrolist        # host tests, including every LVGL batch
make build APP=metrolist       # target/metrolist.wp2app
```

Then install the `.wp2app` with the App Loader. From the phone it is also bundled in the companion: Settings → Watch →
**Install Metrolist on watch** sends the same package over the channel, with no second pairing.

## Building the Android app

Metrolist is Kotlin Multiplatform and the BLE link is Android-only, so the app needs upstream's toolchain: JDK 21,
Android SDK platform `android-37.1`, NDK `30.0.16248370`, CMake `4.1.2`, and the `metroproto` and `libopus` sources
(this repository has them in place, without their `.git`):

```bash
cd android
./gradlew :androidApp:assembleDebug              # androidApp/build/outputs/apk/debug/androidApp-debug.apk
./gradlew :androidApp:testDebugUnitTest --tests 'com.metrolist.android.watch.*'
```

The debug build is `com.metrolist.music.debug`, so it installs next to a release Metrolist. Nothing X keeps working: the
watch is already bonded there and the link uses the app channel, not a second pairing.

## Before calling it shipped

- Nothing X and this app together on each supported vendor/version: no pairing popup, both keep notifications, the
  channel survives screen-off and a service restart. `connectGatt` next to Nothing X is still untested on Android.
- Two accounts: the watch must get nothing from the other one.
- The watch and the phone are one central at a time, so no second device (a Mac, say) while the phone is connected.

## Licences

`apps/` is MIT, like the rest of the FMC apps repository. `android/` is GPL-3.0, inherited from Metrolist; see
`android/LICENSE`.
