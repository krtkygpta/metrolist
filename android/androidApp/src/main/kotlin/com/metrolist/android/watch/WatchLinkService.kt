/*
 * Metrolist Project (C) 2026
 * Licensed under GPL-3.0 | See git history for contributors
 */

package com.metrolist.android.watch

import android.Manifest
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.bluetooth.BluetoothDevice
import android.bluetooth.BluetoothManager
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.content.ContextCompat
import coil3.ImageLoader
import com.metrolist.android.MainActivity
import com.metrolist.android.deviceconnect.AndroidDeviceConnect
import com.metrolist.android.playback.PlayerConnection
import com.metrolist.android.watch.WatchNodeBook.Entry
import com.metrolist.android.watch.WatchNodeBook.Kind
import com.metrolist.shared.db.MusicDatabase
import com.metrolist.shared.model.SongItem
import com.metrolist.shared.state.GlobalYouTubeRepository
import com.metrolist.shared.state.SongLibraryState
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull
import org.koin.mp.KoinPlatform
import kotlin.math.roundToInt

/**
 * Holds the watch link while Settings → Watch is on.
 *
 * The phone stays the only YouTube Music client: the watch names a node and an item and this service answers with
 * names, colours and play state, and applies transport, like and volume commands to the app's own player.
 *
 * A connected-device foreground service keeps the GATT link alive while the app is backgrounded. It never runs while
 * the switch is off.
 */
class WatchLinkService : Service() {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    private lateinit var player: PlayerConnection
    private lateinit var database: MusicDatabase
    private lateinit var imageLoader: ImageLoader

    private val book = WatchNodeBook()
    private var content: WatchContent? = null
    private var kinds: Map<Int, Kind> = emptyMap()
    private var connectJob: Job? = null
    private var installJob: Job? = null
    private var installRequested = false

    @Volatile
    private var channel: WatchChannel? = null

    @Volatile
    private var deviceName: String? = null

    /** The playing track's average artwork colour, RGB565, for the watch's player background. */
    @Volatile
    private var nowColour: Int = 0

    /** Koin is up and the player is available; a service restart before that cannot serve the link. */
    private var ready = false

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        val koin = runCatching { KoinPlatform.getKoin() }.getOrNull()
        if (koin == null) {
            stopSelf()
            return
        }
        player = koin.get()
        database = koin.get()
        imageLoader = koin.get()
        content =
            WatchContent(
                context = this,
                database = database,
                repository = GlobalYouTubeRepository.instance,
                colours = ArtworkColours(this, imageLoader),
            )
        createNotificationChannel()
        ready = true
    }

    override fun onStartCommand(
        intent: Intent?,
        flags: Int,
        startId: Int,
    ): Int {
        if (!ready) {
            // A started service must still enter the foreground once before it stops.
            runCatching { startInForeground() }
            stopSelf()
            return START_NOT_STICKY
        }
        when (intent?.action) {
            ACTION_STOP -> {
                stopLink()
                stopSelf()
                return START_NOT_STICKY
            }

            ACTION_RECONNECT -> reconnect()
            ACTION_INSTALL -> requestInstall()
            else -> start()
        }
        return START_STICKY
    }

    private fun start() {
        if (!WatchSettings.enabled(this)) {
            stopSelf()
            return
        }
        startInForeground()
        if (connectJob?.isActive == true) return
        connectJob = scope.launch { runLink() }
    }

    private fun reconnect() {
        connectJob?.cancel()
        connectJob = null
        installJob?.cancel()
        installJob = null
        installRequested = false
        channel?.let { runCatching { it.close() } }
        channel = null
        start()
    }

    private fun stopLink() {
        connectJob?.cancel()
        connectJob = null
        installJob?.cancel()
        installJob = null
        installRequested = false
        channel?.let { runCatching { it.close() } }
        channel = null
        book.clear()
        WatchLinkState.reset()
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
            stopForeground(STOP_FOREGROUND_REMOVE)
        } else {
            @Suppress("DEPRECATION")
            stopForeground(true)
        }
    }

    override fun onDestroy() {
        stopLink()
        scope.cancel()
        super.onDestroy()
    }

    // ---- the link ----

    private suspend fun runLink() {
        var backoff = 1_000L
        while (currentCoroutineContext().isActive) {
            val device = bondedWatch()
            if (device == null) {
                if (WatchLinkState.status.value.phase != WatchLinkPhase.NO_PERMISSION) {
                    setPhase(WatchLinkPhase.ERROR, detail = "No bonded watch. Pair it once in Nothing X.")
                }
                delay(15_000)
                continue
            }
            deviceName = runCatching { device.name }.getOrNull()
            setPhase(WatchLinkPhase.CONNECTING)
            var link: WatchChannel? = null
            try {
                link = WatchChannel(this, device)
                link.open()
                channel = link
                WatchSettings.setAddress(this, device.address)
                setPhase(WatchLinkPhase.CONNECTED)
                buildContent()
                backoff = 1_000L
                coroutineScope {
                    launch { publishState(link) }
                    launch { ticker(link) }
                    launch { pollEvents(link) }
                    if (installRequested) {
                        installRequested = false
                        installJob = launch { runInstall(link) }
                    }
                    for (frame in link.events) {
                        handle(link, frame)
                    }
                }
                setPhase(WatchLinkPhase.WAITING_FOR_WATCH)
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (error: Exception) {
                setPhase(WatchLinkPhase.ERROR, detail = error.message)
            } finally {
                installJob?.cancel()
                installJob = null
                installRequested = false
                channel = null
                runCatching { link?.close() }
            }
            delay(backoff)
            backoff = (backoff * 2).coerceAtMost(60_000L)
        }
    }

    private suspend fun buildContent() {
        val content = content ?: return
        val built =
            runCatching { content.build() }
                .onFailure { Log.w(TAG, "watch content build failed", it) }
                .getOrNull() ?: return
        kinds = built.kinds
        book.replace(built.root, built.nodes)
        Log.i(
            TAG,
            "watch content built: ${built.nodes.size} nodes, ${built.kinds.size} kinds, " +
                "${built.root.entries.size} root categories",
        )
    }

    private fun requestInstall() {
        if (installJob?.isActive == true) {
            Log.i(TAG, "install already running")
            return
        }
        val current = channel
        if (current != null && current.connected) {
            installJob = scope.launch { runInstall(current) }
        } else {
            installRequested = true
            start()
        }
    }

    private suspend fun publishState(link: WatchChannel) {
        combine(
            player.currentSong,
            player.isPlaying,
            player.currentPosition,
            player.duration,
            player.volume,
        ) { song, playing, position, duration, volume ->
            // Reading the artwork may fail or be slow: it must never take the link down with it.
            val colour = song?.let { s -> runCatching { content?.colourOf(s) }.getOrNull() } ?: 0
            nowColour = colour
            nowPlaying(song, playing, position, duration, volume, colour)
        }.distinctUntilChanged()
            .collect { sendNow(link, it) }
    }

    private suspend fun ticker(link: WatchChannel) {
        while (currentCoroutineContext().isActive) {
            delay(10_000)
            runCatching { link.send(WatchProtocol.heartbeat()) }
        }
    }

    /**
     * The stub emits an EVENT only after the host sends a frame. Poll PING at the reference host's cadence so
     * watch taps do not wait for the 10-second heartbeat.
     */
    private suspend fun pollEvents(link: WatchChannel) {
        while (currentCoroutineContext().isActive) {
            delay(250)
            runCatching { link.ping() }
                .onFailure { Log.w(TAG, "watch event poll failed", it) }
        }
    }

    private suspend fun handle(
        link: WatchChannel,
        frame: ByteArray,
    ) {
        val command = WatchProtocol.parse(frame)
        if (command == null) {
            Log.w(TAG, "dropped watch frame (${frame.size} B): " + frame.joinToString("") { "%02X".format(it) })
            return
        }
        Log.i(TAG, "watch command: $command")
        try {
            WatchLinkState.update { it.copy(lastFrameAt = System.currentTimeMillis()) }
            when (command) {
                WatchProtocol.Command.Hello -> {
                    sendNow(link, snapshot())
                    sendCategories(link)
                }

                is WatchProtocol.Command.Browse -> {
                    if (command.node == WatchProtocol.ROOT_NODE) sendCategories(link) else sendList(link, command.node)
                }

                is WatchProtocol.Command.Play -> play(link, command.node, command.index)

                is WatchProtocol.Command.Transport -> {
                    // The player lives on the main thread and rejects calls from this background coroutine.
                    withContext(Dispatchers.Main.immediate) {
                        when (command.command) {
                            WatchProtocol.CMD_TOGGLE -> player.setPlayingFromRemote(!player.isPlaying.value)
                            WatchProtocol.CMD_NEXT -> player.skipToNextFromRemote()
                            WatchProtocol.CMD_PREV -> player.skipToPreviousFromRemote()
                        }
                    }
                }

                is WatchProtocol.Command.Like -> setLiked(command.on)

                is WatchProtocol.Command.Volume ->
                    withContext(Dispatchers.Main.immediate) {
                        player.setVolumeFromRemote((command.level / 100f).coerceIn(0f, 1f))
                    }

                WatchProtocol.Command.Queue -> sendQueue(link)

                is WatchProtocol.Command.QueuePlay -> playQueue(link, command.index)
            }
            // Authoritative state after every command: the watch never keeps an optimistic value.
            if (command !is WatchProtocol.Command.Hello &&
                command !is WatchProtocol.Command.Browse &&
                command !is WatchProtocol.Command.Queue
            ) {
                sendNow(link, snapshot())
            }
        } catch (cancelled: CancellationException) {
            throw cancelled
        } catch (error: Exception) {
            Log.w(TAG, "watch command failed: $command", error)
        }
    }

    // ---- answers ----

    private fun snapshot(): WatchProtocol.NowPlaying =
        nowPlaying(
            song = player.currentSong.value,
            playing = player.isPlaying.value,
            position = player.currentPosition.value,
            duration = player.duration.value,
            volume = player.volume.value,
            colour = nowColour,
        )

    private fun nowPlaying(
        song: SongItem?,
        playing: Boolean,
        position: Long,
        duration: Long,
        volume: Float,
        colour: Int,
    ): WatchProtocol.NowPlaying =
        WatchProtocol.NowPlaying(
            playing = playing,
            liked = song != null && SongLibraryState.isInLibrary(song),
            hasTrack = song != null,
            positionPercent = if (duration > 0L) ((position * 100L) / duration).toInt().coerceIn(0, 100) else 0,
            volumePercent = (volume * 100f).roundToInt().coerceIn(0, 100),
            title = song?.let { it.customTitle ?: it.title }.orEmpty(),
            artist = song?.artists?.joinToString(", ") { it.name }.orEmpty(),
            colour = colour,
        )

    private suspend fun sendNow(
        link: WatchChannel,
        now: WatchProtocol.NowPlaying,
    ) {
        send(link, WatchProtocol.nowPlaying(now), "now playing")
    }

    /** One frame to the watch; a failure is logged, never thrown at the link loop. */
    private suspend fun send(
        link: WatchChannel,
        frame: ByteArray,
        what: String,
    ) {
        runCatching { link.send(frame) }
            .onFailure { Log.w(TAG, "sending $what failed (${frame.size} B)", it) }
    }

    private suspend fun sendCategories(link: WatchChannel) {
        var root = book.root()
        if (root == null) {
            // The build at connect time can fail (a slow home request, a database still opening): try once more
            // before telling the watch there is nothing.
            buildContent()
            root = book.root()
        }
        if (root == null) {
            Log.w(TAG, "no categories to send: the node book is empty")
            return
        }
        val categories = WatchNodeBook.rootCategories(root, kinds)
        Log.i(TAG, "sending ${categories.size} root categories")
        send(link, WatchProtocol.categories(categories), "categories")
    }

    private suspend fun sendList(
        link: WatchChannel,
        nodeId: Int,
    ) {
        var node = book.node(nodeId)
        if (node == null) {
            node = runCatching { content?.loadChild(nodeId) }.getOrNull()
            node?.let(book::put)
        }
        if (node == null || node.notReady) {
            Log.w(TAG, "browse rejected: node $nodeId is ${if (node == null) "missing" else "not ready"}")
            send(link, WatchProtocol.listNone(nodeId, WatchProtocol.CODE_NOT_READY), "list not ready")
            return
        }
        val rows = node.shown
        send(link, WatchProtocol.listBegin(node.id, node.kind.wire, rows.size, node.title), "list begin")
        var start = 0
        while (start < rows.size) {
            val chunk = rows.drop(start).take(WatchProtocol.rowsPerFrame())
            val frame = WatchProtocol.items(node.id, start, chunk.map(::row))
            if (frame.isEmpty()) break
            send(link, frame, "list items $start")
            start += chunk.size
        }
        send(link, WatchProtocol.listEnd(node.id, rows.size), "list end")
    }

    private fun row(entry: Entry): WatchProtocol.Row =
        when (entry) {
            is Entry.Track -> WatchProtocol.Row(entry.title, entry.artist, entry.colour, null)
            is Entry.Folder -> WatchProtocol.Row(entry.title, entry.artist, entry.colour, entry.node)
        }

    /**
     * The controller's queue page: the first [WatchProtocol.MAX_ITEMS] entries of the phone's queue and the one that
     * plays. No artwork colour is fetched here: twenty covers would stall the link, and the page does not tint rows.
     */
    private suspend fun sendQueue(link: WatchChannel) {
        val songs = player.queueSongs.value.take(WatchProtocol.MAX_ITEMS)
        val current = player.queueState.value.currentIndex.coerceIn(0, (songs.size - 1).coerceAtLeast(0))
        Log.i(TAG, "sending queue: ${songs.size} entries, current $current")
        send(link, WatchProtocol.queueBegin(songs.size, current), "queue begin")
        var start = 0
        while (start < songs.size) {
            val chunk = songs.drop(start).take(WatchProtocol.rowsPerFrame())
            send(link, WatchProtocol.queueItems(start, chunk.map(::queueRow)), "queue items $start")
            start += chunk.size
        }
    }

    private fun queueRow(song: SongItem): WatchProtocol.Row =
        WatchProtocol.Row(
            title = song.customTitle ?: song.title,
            artist = song.artists.joinToString(", ") { it.name },
            colour = 0,
            node = null,
        )

    /** The watch tapped a queue entry: play that position in the queue the phone already has. */
    private fun playQueue(
        link: WatchChannel,
        index: Int,
    ) {
        val songs = player.queueSongs.value
        val song = songs.getOrNull(index)
        if (song == null) {
            Log.w(TAG, "queue play rejected: no entry $index of ${songs.size}")
            scope.launch { send(link, WatchProtocol.ack(WatchProtocol.ACK_REFUSED), "play ack") }
            return
        }
        Log.i(TAG, "queue play: index=$index song=${song.id}")
        scope.launch {
            try {
                withContext(Dispatchers.Main.immediate) { player.skipToQueueItem(index) }
                // The ack only goes out once that entry really plays, as for a tap in a list.
                val switched =
                    withTimeoutOrNull(QUEUE_SWITCH_MS) { player.currentSong.first { it?.id == song.id } } != null
                send(
                    link,
                    WatchProtocol.ack(if (switched) WatchProtocol.ACK_PLAYING else WatchProtocol.ACK_NO_SWITCH),
                    "play ack",
                )
                // The current entry moved: the page's filled row follows it.
                if (switched) sendQueue(link)
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (error: Exception) {
                Log.w(TAG, "queue play failed: index=$index", error)
                runCatching { send(link, WatchProtocol.ack(WatchProtocol.ACK_NO_SWITCH), "play ack") }
            }
        }
    }

    // ---- bundled watch package ----

    private fun setInstallState(
        message: String,
        percent: Int? = null,
    ) {
        WatchLinkState.update {
            it.copy(
                installMessage = message,
                installPercent = percent,
            )
        }
    }

    /** Uploads the `.wp2app` bundled in this APK with the same INFO/BEGIN/DATA/END flow as wp2apps install. */
    private suspend fun runInstall(link: WatchChannel) {
        setInstallState("Preparing Metrolist package")
        try {
            val data = assets.open(PACKAGE_ASSET).use { it.readBytes() }
            val header = WatchInstall.parsePackage(data)
            if (header.kind != WatchInstall.KIND_APP) {
                throw IllegalStateException("this bundled file is a firmware/loader package, not an app")
            }
            if (link.mtuBytes() < 160) {
                throw IllegalStateException("the watch kept a ${link.mtuBytes()}-byte MTU; retry after it settles")
            }

            setInstallState("Checking watch firmware")
            val stub = link.call(FirmwareChannel.INFO, byteArrayOf(WatchInstall.STUB_SLOT.toByte()))
            if (stub != null && stub.isNotEmpty() && stub[0].toInt() != 0 && stub[0].toInt() != 1) {
                throw IllegalStateException(
                    "watch refused firmware check: " + WatchInstall.statusName(stub[0].toInt() and 255),
                )
            }
            WatchInstall.stubAbiMinor(stub ?: ByteArray(0))?.let { offered ->
                if (header.abiMinor > offered) {
                    throw IllegalStateException(
                        "watch stub is too old: package needs ABI minor ${header.abiMinor}, stub offers $offered",
                    )
                }
            }

            val used = HashMap<Int, String>()
            for (slot in 0 until WatchInstall.SLOTS) {
                val reply = link.call(FirmwareChannel.INFO, byteArrayOf(slot.toByte())) ?: continue
                if (reply.size > 1 && reply[0].toInt() == 0) {
                    used[slot] =
                        try {
                            WatchInstall.parseHeader(reply.copyOfRange(1, minOf(reply.size, 1 + WatchInstall.HEADER_SIZE))).name
                        } catch (_: Exception) {
                            "unknown"
                        }
                }
            }
            val slot =
                WatchInstall.chooseSlot(header.name, used)
                    ?: throw IllegalStateException("no free slot; remove an app on the watch first")
            setInstallState("Uploading ${header.name} to slot $slot", 0)

            val begin =
                link.call(
                    FirmwareChannel.BEGIN,
                    byteArrayOf(slot.toByte()) + WatchInstall.u32le(data.size.toLong()) + WatchInstall.u32le(WatchInstall.crc32(data)),
                ) ?: throw IllegalStateException("watch did not answer BEGIN")
            if (begin[0].toInt() != 0) {
                throw IllegalStateException(WatchInstall.statusName(begin[0].toInt() and 255))
            }

            var offset = 0
            var lastPercent = 0
            while (offset < data.size) {
                val count = minOf(WatchInstall.MAX_DATA, data.size - offset)
                val part = data.copyOfRange(offset, offset + count)
                val reply =
                    link.call(
                        FirmwareChannel.DATA,
                        WatchInstall.u32le(offset.toLong()) + part,
                    ) ?: throw IllegalStateException("upload stalled after $offset bytes")
                if (reply[0].toInt() != 0) {
                    throw IllegalStateException(WatchInstall.statusName(reply[0].toInt() and 255))
                }
                if (reply.size < 5) throw IllegalStateException("short DATA reply from the watch")
                val acked = WatchInstall.le32(reply, 1)
                if (acked != (offset + count).toLong()) {
                    throw IllegalStateException("watch expects offset $acked, sent ${offset + count}")
                }
                offset += count
                val percent = offset * 100 / data.size
                if (percent >= lastPercent + 10 || offset == data.size) {
                    lastPercent = percent
                    setInstallState("Uploading ${header.name} to slot $slot", percent)
                }
            }

            val end = link.call(FirmwareChannel.END, byteArrayOf(0))
                ?: throw IllegalStateException("watch did not answer END")
            if (end[0].toInt() != 0) {
                throw IllegalStateException(WatchInstall.statusName(end[0].toInt() and 255))
            }

            setInstallState("Verifying installed package")
            val verify = link.call(FirmwareChannel.INFO, byteArrayOf(slot.toByte()))
            val installed =
                verify?.takeIf { it.size > 1 && it[0].toInt() == 0 }
                    ?.let { WatchInstall.parseHeader(it.copyOfRange(1, minOf(it.size, 1 + WatchInstall.HEADER_SIZE))).name }
            if (installed != header.name) {
                throw IllegalStateException("watch reports slot $slot as ${installed ?: "empty"}")
            }
            setInstallState("Installed ${header.name} v${WatchInstall.versionText(header.version)} to slot $slot")
            Log.i(TAG, "installed ${header.name} v${WatchInstall.versionText(header.version)} to slot $slot")
        } catch (cancelled: CancellationException) {
            throw cancelled
        } catch (error: Exception) {
            Log.w(TAG, "watch package install failed", error)
            setInstallState("Install failed: ${error.message ?: error.javaClass.simpleName}")
        }
    }

    // ---- commands ----

    private suspend fun play(
        link: WatchChannel,
        nodeId: Int,
        index: Int,
    ) {
        var node = book.node(nodeId)
        if (node == null) {
            // A reconnect clears the book; rebuild once before rejecting, so a tap right after a reconnect works.
            Log.w(TAG, "play: node $nodeId is missing; rebuilding the watch book once")
            buildContent()
            node = book.node(nodeId)
            if (node == null) {
                Log.w(TAG, "play rejected: node $nodeId is still missing after the rebuild")
                send(link, WatchProtocol.ack(WatchProtocol.ACK_REFUSED), "play ack")
                return
            }
        }
        // The watch sends the index of the row it drew, including any folder rows before the song. Keep that
        // mapping instead of filtering folders first, which silently shifted the selected song in mixed lists.
        val visible = node.entries.take(WatchProtocol.MAX_ITEMS)
        val selectedEntry = visible.getOrNull(index)
        if (selectedEntry !is Entry.Track) {
            Log.w(TAG, "play rejected: node $nodeId row $index is not a song")
            send(link, WatchProtocol.ack(WatchProtocol.ACK_REFUSED), "play ack")
            return
        }
        val songs = visible.filterIsInstance<Entry.Track>().map { it.song }
        val songIndex = visible.take(index).count { it is Entry.Track }
        if (songIndex !in songs.indices || songs[songIndex].id != selectedEntry.song.id) {
            Log.w(TAG, "play rejected: node $nodeId row $index mapped to queue index $songIndex")
            send(link, WatchProtocol.ack(WatchProtocol.ACK_REFUSED), "play ack")
            return
        }
        Log.i(TAG, "play: node=$nodeId index=$index")
        scope.launch {
            try {
                val selected = songs[songIndex]
                if (selected.isUnavailable) {
                    Log.w(TAG, "play refused: node=$nodeId index=$songIndex is unavailable")
                    send(link, WatchProtocol.ack(WatchProtocol.ACK_REFUSED), "play ack")
                    return@launch
                }
                // The watch owns this replacement: it must beat every interceptor and always start playing.
                AndroidDeviceConnect.manager.remote.let { remote ->
                    Log.i(TAG, "device connect: active=" + remote.isActive + " following=" + remote.isFollowing)
                }
                Log.i(TAG, "play requested: node=$nodeId row=$index queueIndex=$songIndex song=${selected.id}")
                // The ack only goes out when the requested song really became the current one, so the watch's
                // "Playing..." tells the truth about the whole chain: frame received, processed, playback switched.
                val switched = player.playSongFromWatch(selected)
                Log.i(TAG, "play " + if (switched) "confirmed" else "NOT confirmed: song=${selected.id}")
                send(link, WatchProtocol.ack(if (switched) WatchProtocol.ACK_PLAYING else WatchProtocol.ACK_NO_SWITCH), "play ack")
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (error: Exception) {
                Log.w(TAG, "play failed: node=$nodeId index=$songIndex", error)
                runCatching { send(link, WatchProtocol.ack(WatchProtocol.ACK_NO_SWITCH), "play ack") }
            }
        }
    }

    private fun setLiked(on: Boolean) {
        val song = player.currentSong.value
        if (song == null) {
            Log.w(TAG, "like rejected: no current track")
            return
        }
        if (SongLibraryState.isInLibrary(song) != on) SongLibraryState.toggle(song)
    }

    // ---- Android plumbing ----

    private fun bondedWatch(): BluetoothDevice? {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S &&
            ContextCompat.checkSelfPermission(this, Manifest.permission.BLUETOOTH_CONNECT) !=
            PackageManager.PERMISSION_GRANTED
        ) {
            setPhase(WatchLinkPhase.NO_PERMISSION)
            return null
        }
        val adapter = (getSystemService(BLUETOOTH_SERVICE) as? BluetoothManager)?.adapter ?: return null
        val bonded = runCatching { adapter.bondedDevices?.toList().orEmpty() }.getOrDefault(emptyList())
        val saved = WatchSettings.address(this)
        return bonded.firstOrNull { it.address == saved }
            ?: bonded.firstOrNull { it.name?.contains("watch", ignoreCase = true) == true }
    }

    private fun setPhase(
        phase: WatchLinkPhase,
        detail: String? = null,
    ) {
        WatchLinkState.update { it.copy(phase = phase, detail = detail, deviceName = deviceName) }
    }

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val manager = getSystemService(NotificationManager::class.java) ?: return
        manager.createNotificationChannel(
            NotificationChannel(CHANNEL_ID, NOTIFICATION_TITLE, NotificationManager.IMPORTANCE_LOW).apply {
                description = "Keeps the watch connected while Metrolist runs in the background."
                setShowBadge(false)
            },
        )
    }

    private fun startInForeground() {
        val notification: Notification =
            NotificationCompat
                .Builder(this, CHANNEL_ID)
                .setContentTitle(NOTIFICATION_TITLE)
                .setContentText("Connected to your watch")
                .setSmallIcon(com.metrolist.android.R.mipmap.ic_launcher)
                .setOngoing(true)
                .setPriority(NotificationCompat.PRIORITY_LOW)
                .setContentIntent(
                    android.app.PendingIntent.getActivity(
                        this,
                        0,
                        Intent(this, MainActivity::class.java),
                        android.app.PendingIntent.FLAG_IMMUTABLE,
                    ),
                ).build()
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            startForeground(NOTIFICATION_ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE)
        } else {
            startForeground(NOTIFICATION_ID, notification)
        }
    }

    companion object {
        private const val TAG = "MetrolistWatch"
        private const val CHANNEL_ID = "metrolist_watch_link"
        private const val NOTIFICATION_ID = 0x77A7
        private const val NOTIFICATION_TITLE = "Metrolist watch link"
        private const val PACKAGE_ASSET = "metrolist.wp2app"

        /** How long a queue tap has to become the current song before the watch is told it did not switch. */
        private const val QUEUE_SWITCH_MS = 5_000L

        private const val ACTION_START = "com.metrolist.music.action.WATCH_LINK_START"
        private const val ACTION_STOP = "com.metrolist.music.action.WATCH_LINK_STOP"
        private const val ACTION_RECONNECT = "com.metrolist.music.action.WATCH_LINK_RECONNECT"
        private const val ACTION_INSTALL = "com.metrolist.music.action.WATCH_LINK_INSTALL"

        /** Turns the switch on and starts holding the link. */
        fun start(context: Context) {
            WatchSettings.setEnabled(context, true)
            WatchLinkState.update { it.copy(phase = WatchLinkPhase.CONNECTING, detail = null) }
            intent(context, ACTION_START)
        }

        /** Turns the switch off and drops the link. */
        fun stop(context: Context) {
            WatchSettings.setEnabled(context, false)
            WatchLinkState.reset()
            context.stopService(Intent(context, WatchLinkService::class.java))
        }

        /** Test connection: drops the current link and opens a fresh one. */
        fun reconnect(context: Context) {
            if (!WatchSettings.enabled(context)) {
                start(context)
                return
            }
            intent(context, ACTION_RECONNECT)
        }

        /** Starts or queues an install of the bundled Metrolist watch package from Settings → Watch. */
        fun install(context: Context) {
            if (!WatchSettings.enabled(context)) {
                start(context)
            }
            intent(context, ACTION_INSTALL)
        }

        private fun intent(
            context: Context,
            action: String,
        ) {
            val service = Intent(context, WatchLinkService::class.java).setAction(action)
            ContextCompat.startForegroundService(context, service)
        }
    }
}
