/*
 * Metrolist Project (C) 2026
 * Licensed under GPL-3.0 | See git history for contributors
 */

package com.metrolist.android.watch

import java.io.ByteArrayOutputStream
import kotlin.math.min

/**
 * The `M1` link between the CMF Watch Pro 2 app (`apps/metrolist` in the CMF workspace) and this fork.
 *
 * The wire format is ASCII, one kind byte, then fixed fields; a frame is at most 128 bytes. The watch names a node
 * to browse and an item to play, and this side answers with what it has. Nothing here touches the account, a token,
 * a cookie or a file path: only names the user already sees, colours and play state cross the link.
 *
 * Keep this file free of Android imports: it is covered by host unit tests.
 */
internal object WatchProtocol {
    const val MAX_MESSAGE = 128
    const val MAX_ITEMS = 20
    const val MAX_ROOT = 6
    const val MAX_TITLE = 34
    const val MAX_ARTIST = 20
    const val MAX_NAME = 20

    const val ROOT_NODE = 0
    const val KIND_FOLDER = 0
    const val KIND_SONGS = 1

    const val FLAG_PLAYING = 0x01
    const val FLAG_LIKED = 0x02
    const val FLAG_HAS_TRACK = 0x04

    const val CODE_EMPTY = 0
    const val CODE_ERROR = 1
    const val CODE_NOT_READY = 2

    const val CMD_TOGGLE = 0
    const val CMD_NEXT = 1
    const val CMD_PREV = 2

    private val MAGIC_A = 'M'.code.toByte()
    private val MAGIC_B = '1'.code.toByte()

    private val HELLO = '?'.code.toByte()
    private val BROWSE = 'b'.code.toByte()
    private val PLAY = 'p'.code.toByte()
    private val TRANSPORT = 't'.code.toByte()
    private val LIKE = 'l'.code.toByte()
    private val VOLUME = 'v'.code.toByte()
    private val QUEUE_REQ = 'q'.code.toByte()
    private val QUEUE_PLAY = 'j'.code.toByte()

    private val NOW = 'S'.code.toByte()
    private val CATEGORIES = 'C'.code.toByte()
    private val LIST_BEGIN = 'B'.code.toByte()
    private val ITEMS = 'I'.code.toByte()
    private val LIST_END = 'E'.code.toByte()
    private val LIST_NONE = 'X'.code.toByte()
    private val QUEUE = 'Q'.code.toByte()
    private val QUEUE_ITEMS = 'J'.code.toByte()
    private val HEARTBEAT = 'H'.code.toByte()
    private val ACK = 'K'.code.toByte()

    /** `K` ack codes sent to the watch after a play command. */
    const val ACK_PLAYING = 0
    const val ACK_REFUSED = 1
    const val ACK_NO_SWITCH = 2

    /** One root category, or one folder entry. */
    internal data class Category(
        val id: Int,
        val kind: Int,
        val name: String,
        /** RGB565, little endian on the wire. */
        val colour: Int,
    )

    /** One row of a list. */
    internal data class Row(
        val title: String,
        val artist: String,
        val colour: Int,
        /** A folder's node id, or `null` for a song. */
        val node: Int?,
    )

    internal data class NowPlaying(
        val playing: Boolean,
        val liked: Boolean,
        val hasTrack: Boolean,
        /** 0..=100. */
        val positionPercent: Int,
        /** 0..=100. */
        val volumePercent: Int,
        val title: String,
        val artist: String,
        /** The artwork's average colour, RGB565, for the player's background; 0 when there is none. */
        val colour: Int,
    )

    /** What the watch asked for. */
    internal sealed interface Command {
        data object Hello : Command

        data class Browse(val node: Int) : Command

        data class Play(val node: Int, val index: Int) : Command

        data class Transport(val command: Int) : Command

        data class Like(val on: Boolean) : Command

        data class Volume(val level: Int) : Command

        /** The controller's queue page: send the current queue. */
        data object Queue : Command

        /** Play queue entry `index` of the queue the phone already has. */
        data class QueuePlay(val index: Int) : Command
    }

    /**
     * `text` clipped to at most `max` UTF-8 bytes without splitting a character.
     */
    fun clip(
        text: String,
        max: Int,
    ): ByteArray {
        val bytes = text.toByteArray(Charsets.UTF_8)
        if (bytes.size <= max) return bytes
        var end = max
        while (end > 0 && (bytes[end].toInt() and 0xC0) == 0x80) end--
        return bytes.copyOf(end)
    }

    fun rgb565(colour: Int): Int {
        val r = (colour shr 16) and 0xFF
        val g = (colour shr 8) and 0xFF
        val b = colour and 0xFF
        return ((r shr 3) shl 11) or ((g shr 2) shl 5) or (b shr 3)
    }

    fun nowPlaying(now: NowPlaying): ByteArray {
        val title = clip(now.title, MAX_TITLE)
        val artist = clip(now.artist, MAX_ARTIST)
        var flags = 0
        if (now.playing) flags = flags or FLAG_PLAYING
        if (now.liked) flags = flags or FLAG_LIKED
        if (now.hasTrack) flags = flags or FLAG_HAS_TRACK
        val out = ByteArrayOutputStream(10 + title.size + artist.size)
        out.write(MAGIC_A.toInt())
        out.write(MAGIC_B.toInt())
        out.write(NOW.toInt())
        out.write(flags)
        out.write(now.positionPercent.coerceIn(0, 100))
        out.write(now.volumePercent.coerceIn(0, 100))
        out.write(title.size)
        out.write(title)
        out.write(artist.size)
        out.write(artist)
        writeU16(out, now.colour and 0xFFFF)
        return out.toByteArray()
    }

    fun categories(categories: List<Category>): ByteArray {
        val kept = categories.take(MAX_ROOT)
        // 4 header bytes, then 5 per category (id, kind, name length, colour: u16): the names share what is left.
        val nameBudget = if (kept.isEmpty()) 0 else (MAX_MESSAGE - 4 - kept.size * 5) / kept.size
        val out = ByteArrayOutputStream(MAX_MESSAGE)
        out.write(MAGIC_A.toInt())
        out.write(MAGIC_B.toInt())
        out.write(CATEGORIES.toInt())
        out.write(kept.size)
        for (category in kept) {
            val name = clip(category.name, min(MAX_NAME, nameBudget))
            out.write(category.id)
            out.write(category.kind)
            out.write(name.size)
            out.write(name)
            writeU16(out, category.colour and 0xFFFF)
        }
        return out.toByteArray()
    }

    fun listBegin(
        node: Int,
        kind: Int,
        total: Int,
        title: String,
    ): ByteArray {
        val name = clip(title, MAX_NAME)
        val out = ByteArrayOutputStream(8 + name.size)
        out.write(MAGIC_A.toInt())
        out.write(MAGIC_B.toInt())
        out.write(LIST_BEGIN.toInt())
        out.write(node)
        out.write(kind)
        out.write(total.coerceIn(0, 0xFF))
        out.write(name.size)
        out.write(name)
        return out.toByteArray()
    }

    /** One `I` frame: at most [MAX_ITEMS] rows, two at a time by the caller. */
    fun items(
        node: Int,
        start: Int,
        rows: List<Row>,
    ): ByteArray {
        val kept = rows.take(MAX_ITEMS - start).ifEmpty { return ByteArray(0) }
        val out = ByteArrayOutputStream(MAX_MESSAGE)
        out.write(MAGIC_A.toInt())
        out.write(MAGIC_B.toInt())
        out.write(ITEMS.toInt())
        out.write(node)
        out.write(start)
        out.write(kept.size)
        for (row in kept) {
            val title = clip(row.title, MAX_TITLE)
            val artist = clip(row.artist, MAX_ARTIST)
            out.write(title.size)
            out.write(title)
            out.write(artist.size)
            out.write(artist)
            writeU16(out, row.colour and 0xFFFF)
            out.write(row.node ?: NONE)
        }
        return out.toByteArray()
    }

    fun listEnd(
        node: Int,
        total: Int,
    ): ByteArray = byteArrayOf(MAGIC_A, MAGIC_B, LIST_END, node.toByte(), total.coerceIn(0, 0xFF).toByte())

    fun listNone(
        node: Int,
        code: Int,
    ): ByteArray = byteArrayOf(MAGIC_A, MAGIC_B, LIST_NONE, node.toByte(), code.coerceIn(0, 2).toByte())

    /** The queue starts: `total` entries follow (at most [MAX_ITEMS]), `current` plays. */
    fun queueBegin(
        total: Int,
        current: Int,
    ): ByteArray =
        byteArrayOf(
            MAGIC_A,
            MAGIC_B,
            QUEUE,
            total.coerceIn(0, MAX_ITEMS).toByte(),
            current.coerceIn(0, MAX_ITEMS - 1).toByte(),
        )

    /** One `J` frame: queue entries `start..start+n`, without a node id. */
    fun queueItems(
        start: Int,
        rows: List<Row>,
    ): ByteArray {
        val kept = rows.take(MAX_ITEMS - start).ifEmpty { return ByteArray(0) }
        val out = ByteArrayOutputStream(MAX_MESSAGE)
        out.write(MAGIC_A.toInt())
        out.write(MAGIC_B.toInt())
        out.write(QUEUE_ITEMS.toInt())
        out.write(start)
        out.write(kept.size)
        for (row in kept) {
            val title = clip(row.title, MAX_TITLE)
            val artist = clip(row.artist, MAX_ARTIST)
            out.write(title.size)
            out.write(title)
            out.write(artist.size)
            out.write(artist)
            writeU16(out, row.colour and 0xFFFF)
        }
        return out.toByteArray()
    }

    fun heartbeat(): ByteArray = byteArrayOf(MAGIC_A, MAGIC_B, HEARTBEAT)

    /** Tells the watch what a play command did: switched, refused, or failed to switch. */
    fun ack(code: Int): ByteArray = byteArrayOf(MAGIC_A, MAGIC_B, ACK, code.toByte())

    /** The largest number of rows that still fits one `I` frame with full-length titles. */
    fun rowsPerFrame(): Int = 2

    /** A watch frame, or `null` when it is malformed or not ours. */
    fun parse(message: ByteArray): Command? {
        if (message.size < 3 || message.size > MAX_MESSAGE) return null
        if (message[0] != MAGIC_A || message[1] != MAGIC_B) return null
        return when (message[2]) {
            HELLO -> if (message.size == 3) Command.Hello else null
            BROWSE -> if (message.size == 4) Command.Browse(message[3].u8()) else null
            PLAY -> if (message.size == 5) Command.Play(message[3].u8(), message[4].u8()) else null
            TRANSPORT ->
                if (message.size == 4 && message[3].u8() in CMD_TOGGLE..CMD_PREV) {
                    Command.Transport(message[3].u8())
                } else {
                    null
                }

            LIKE -> if (message.size == 4 && message[3].u8() <= 1) Command.Like(message[3].u8() == 1) else null
            VOLUME -> if (message.size == 4 && message[3].u8() <= 100) Command.Volume(message[3].u8()) else null
            QUEUE_REQ -> if (message.size == 3) Command.Queue else null
            QUEUE_PLAY ->
                if (message.size == 4 && message[3].u8() < MAX_ITEMS) {
                    Command.QueuePlay(message[3].u8())
                } else {
                    null
                }

            else -> null
        }
    }

    const val NONE = 0xFF

    private fun Byte.u8(): Int = toInt() and 0xFF

    private fun writeU16(
        out: ByteArrayOutputStream,
        value: Int,
    ) {
        out.write(value and 0xFF)
        out.write((value shr 8) and 0xFF)
    }
}
