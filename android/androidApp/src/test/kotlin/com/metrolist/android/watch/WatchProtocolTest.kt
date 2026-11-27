/*
 * Metrolist Project (C) 2026
 * Licensed under GPL-3.0 | See git history for contributors
 */

package com.metrolist.android.watch

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class WatchProtocolTest {
    private fun ascii(bytes: ByteArray): String = String(bytes, Charsets.ISO_8859_1)

    @Test
    fun nowPlayingCarriesFlagsAndClippedText() {
        val frame =
            WatchProtocol.nowPlaying(
                WatchProtocol.NowPlaying(
                    playing = true,
                    liked = true,
                    hasTrack = true,
                    positionPercent = 42,
                    volumePercent = 70,
                    title = "x".repeat(80),
                    artist = "a".repeat(80),
                    colour = 0x1F3E,
                ),
            )
        assertTrue(frame.size <= WatchProtocol.MAX_MESSAGE)
        assertEquals("M1S", ascii(frame.copyOfRange(0, 3)))
        assertEquals(0x07, frame[3].toInt())
        assertEquals(42, frame[4].toInt())
        assertEquals(70, frame[5].toInt())
        assertEquals(WatchProtocol.MAX_TITLE, frame[6].toInt())
        val artistLengthAt = 7 + WatchProtocol.MAX_TITLE
        assertEquals(WatchProtocol.MAX_ARTIST, frame[artistLengthAt].toInt())
        // The artwork colour is the last two bytes, little endian.
        val colourAt = artistLengthAt + 1 + WatchProtocol.MAX_ARTIST
        assertEquals(0x1F3E, (frame[colourAt].toInt() and 0xFF) or ((frame[colourAt + 1].toInt() and 0xFF) shl 8))
        assertEquals(frame.size, colourAt + 2)
    }

    @Test
    fun anEventEnvelopeIsStrippedBeforeParsing() {
        // docs/abi-1.1.md, "Data channel": an EVENT plain is `seq | slot | data`, and the reference host
        // strips the same two bytes (wp2apps.channel.message: Message(plain[0], plain[1], plain[2:])).
        // Handing the envelope to the M1 parser made every watch frame look like it began with `seq`/`slot`
        // instead of `M1`, so all of them were dropped: no categories, no transport.
        val data = "M1t\u0001".toByteArray()
        val plain = byteArrayOf(0x2A, 0x00) + data
        assertNull("the envelope alone parses as nothing", WatchProtocol.parse(plain))
        val stripped = FirmwareChannel.appData(plain)
        assertEquals(WatchProtocol.Command.Transport(WatchProtocol.CMD_NEXT), WatchProtocol.parse(stripped!!))
        // Fewer than seq, slot and one data byte is not an app message.
        assertNull(FirmwareChannel.appData(byteArrayOf(1, 2)))
        assertNull(FirmwareChannel.appData(ByteArray(0)))
    }

    @Test
    fun aSearchRequestCarriesItsQuery() {
        val frame = "M1f".toByteArray() + byteArrayOf(9) + "daft punk".toByteArray()
        assertEquals(WatchProtocol.Command.Search("daft punk"), WatchProtocol.parse(frame))
        assertNull(WatchProtocol.parse("M1f".toByteArray()))
        // An empty query is not a search: the watch only sends a name.
        assertNull(WatchProtocol.parse("M1f\u0000".toByteArray()))
        // The length has to match the frame exactly, or the query is read past its end.
        assertNull(WatchProtocol.parse("M1f\u0003abc!".toByteArray()))
    }

    @Test
    fun aTitleIsReducedToWhatTheWatchCanDraw() {
        // Accents fold, curly quotes and dashes become ASCII, and what has no glyph on the watch is dropped: the stock
        // fonts would otherwise draw a missing character as nothing (or a `?`) in the middle of a song name.
        assertEquals("Don't Stop - Me Now", WatchProtocol.plain("Don\u2019t\u00A0Stop \u2013 Me Now"))
        assertEquals("Cafe del Mar", WatchProtocol.plain("Caf\u00e9 del Mar"))
        assertEquals("Song name", WatchProtocol.plain("Song \uD83D\uDD25 name"))
        assertEquals("a...b", WatchProtocol.plain("a\u2026b"))
        assertEquals("", WatchProtocol.plain("\u65E5\u672C\u8A9E"))
        // Every frame leaves here as printable ASCII, so a clip can no longer cut a character in half.
        val clipped = WatchProtocol.clip("aaaaaaaa\u00e9", 4)
        assertEquals("aaaa", String(clipped, Charsets.UTF_8))
        assertTrue(clipped.all { it in 0x20..0x7E })
    }

    @Test
    fun eightCategoriesStillFitOneMessage() {
        val categories =
            (1..8).map { index ->
                WatchProtocol.Category(
                    id = index,
                    kind = if (index > 4) WatchProtocol.KIND_FOLDER else WatchProtocol.KIND_SONGS,
                    // Full-length names: the budget, not the names, has to keep the frame on the wire.
                    name = "Category number $index",
                    colour = 0x1234,
                )
            }
        val frame = WatchProtocol.categories(categories)
        assertTrue("frame is ${frame.size} bytes", frame.size <= WatchProtocol.MAX_MESSAGE)
        assertEquals(8, frame[3].toInt())
        var offset = 4
        repeat(8) {
            val nameLength = frame[offset + 2].toInt() and 0xFF
            assertTrue("name is $nameLength bytes", nameLength <= WatchProtocol.MAX_NAME)
            offset += 5 + nameLength
        }
        assertEquals(frame.size, offset)
        assertEquals(WatchProtocol.CODE_EMPTY, WatchProtocol.listNone(3, 0)[4].toInt())
    }

    @Test
    fun everyShelfNameSurvivesWhenTheFrameFits() {
        // The old even split cut "Quick picks" to ten bytes while "Explore" left seven unused.
        val shelves =
            listOf(
                "Quick picks",
                "Listen again",
                "Downloads",
                "Library",
                "Explore",
                "New releases",
                "Trending now",
                "Made for you",
            )
        val frame =
            WatchProtocol.categories(
                shelves.mapIndexed { index, name ->
                    WatchProtocol.Category(index + 1, WatchProtocol.KIND_FOLDER, name, 0x3A60)
                },
            )
        assertTrue("frame is ${frame.size} bytes", frame.size <= WatchProtocol.MAX_MESSAGE)
        var at = 4
        val names = ArrayList<String>()
        repeat(shelves.size) {
            val length = frame[at + 2].toInt() and 0xFF
            names += String(frame, at + 3, length, Charsets.UTF_8)
            at += 5 + length
        }
        assertEquals(at, frame.size)
        assertEquals(shelves, names)
    }

    @Test
    fun twoFullRowsFitOneItemFrame() {
        val rows =
            listOf(
                WatchProtocol.Row("x".repeat(WatchProtocol.MAX_TITLE), "y".repeat(WatchProtocol.MAX_ARTIST), 0x1234, null),
                WatchProtocol.Row("z".repeat(WatchProtocol.MAX_TITLE), "w".repeat(WatchProtocol.MAX_ARTIST), 0x5678, 9),
            )
        val frame = WatchProtocol.items(1, 0, rows)
        assertTrue("frame is ${frame.size} bytes", frame.size <= WatchProtocol.MAX_MESSAGE)
        assertEquals("M1I", ascii(frame.copyOfRange(0, 3)))
        assertEquals(1, frame[3].toInt())
        assertEquals(0, frame[4].toInt())
        assertEquals(2, frame[5].toInt())
        // Row ids: a song is 0xFF, a folder is the node to open.
        var offset = 6

        fun rowId(): Int {
            val titleLength = frame[offset].toInt() and 0xFF
            offset += 1 + titleLength
            val artistLength = frame[offset].toInt() and 0xFF
            offset += 1 + artistLength
            offset += 2
            val id = frame[offset].toInt() and 0xFF
            offset += 1
            return id
        }
        assertEquals(0xFF, rowId())
        assertEquals(9, rowId())
        assertEquals(frame.size, offset)
    }

    @Test
    fun aQueueFrameFitsTwoFullRowsAndStopsAtTwenty() {
        val rows =
            listOf(
                WatchProtocol.Row("x".repeat(WatchProtocol.MAX_TITLE), "y".repeat(WatchProtocol.MAX_ARTIST), 0, null),
                WatchProtocol.Row("z".repeat(WatchProtocol.MAX_TITLE), "w".repeat(WatchProtocol.MAX_ARTIST), 0, null),
            )
        val frame = WatchProtocol.queueItems(0, rows)
        assertTrue("frame is ${frame.size} bytes", frame.size <= WatchProtocol.MAX_MESSAGE)
        assertEquals("M1J", ascii(frame.copyOfRange(0, 3)))
        assertEquals(0, frame[3].toInt())
        assertEquals(2, frame[4].toInt())
        assertTrue(WatchProtocol.queueItems(20, rows).isEmpty())
        val begin = WatchProtocol.queueBegin(3, 1)
        assertEquals("M1Q", ascii(begin.copyOfRange(0, 3)))
        assertEquals(3, begin[3].toInt())
        assertEquals(1, begin[4].toInt())
    }

    @Test
    fun itemFramesAreEmptyPastTheTwentiethRow() {
        assertTrue(WatchProtocol.items(1, 20, listOf(WatchProtocol.Row("a", "b", 0, null))).isEmpty())
    }

    @Test
    fun listFramesCarryTheNode() {
        assertEquals("M1B", ascii(WatchProtocol.listBegin(4, WatchProtocol.KIND_FOLDER, 3, "Library").copyOfRange(0, 3)))
        assertEquals("M1E", ascii(WatchProtocol.listEnd(4, 3).copyOfRange(0, 3)))
        assertEquals("M1X", ascii(WatchProtocol.listNone(4, 2).copyOfRange(0, 3)))
        assertEquals(2, WatchProtocol.listNone(4, 9)[4].toInt())
        assertEquals("M1H", ascii(WatchProtocol.heartbeat()))
    }

    @Test
    fun playAcksCarryTheirCode() {
        assertEquals("M1K", ascii(WatchProtocol.ack(WatchProtocol.ACK_PLAYING).copyOfRange(0, 3)))
        assertEquals(0, WatchProtocol.ack(WatchProtocol.ACK_PLAYING)[3].toInt())
        assertEquals(1, WatchProtocol.ack(WatchProtocol.ACK_REFUSED)[3].toInt())
        assertEquals(2, WatchProtocol.ack(WatchProtocol.ACK_NO_SWITCH)[3].toInt())
    }

    @Test
    fun watchCommandsParse() {
        assertEquals(WatchProtocol.Command.Hello, WatchProtocol.parse("M1?".toByteArray()))
        assertEquals(WatchProtocol.Command.Browse(0), WatchProtocol.parse("M1b\u0000".toByteArray()))
        assertEquals(WatchProtocol.Command.Browse(200), WatchProtocol.parse(byteArrayOf(77, 49, 98, 200.toByte())))
        assertEquals(WatchProtocol.Command.Play(4, 7), WatchProtocol.parse(byteArrayOf(77, 49, 112, 4, 7)))
        assertEquals(WatchProtocol.Command.Transport(2), WatchProtocol.parse(byteArrayOf(77, 49, 116, 2)))
        assertEquals(WatchProtocol.Command.Like(true), WatchProtocol.parse(byteArrayOf(77, 49, 108, 1)))
        assertEquals(WatchProtocol.Command.Volume(100), WatchProtocol.parse(byteArrayOf(77, 49, 118, 100)))
        assertEquals(WatchProtocol.Command.Queue, WatchProtocol.parse("M1q".toByteArray()))
        assertEquals(WatchProtocol.Command.QueuePlay(7), WatchProtocol.parse(byteArrayOf(77, 49, 106, 7)))
    }

    @Test
    fun malformedFramesAreIgnored() {
        assertNull(WatchProtocol.parse(ByteArray(0)))
        assertNull(WatchProtocol.parse("X1?".toByteArray()))
        assertNull(WatchProtocol.parse(byteArrayOf(77, 49, 63, 0)))
        assertNull(WatchProtocol.parse("M1b".toByteArray()))
        assertNull(WatchProtocol.parse("M1b\u0000\u0000".toByteArray()))
        assertNull(WatchProtocol.parse(byteArrayOf(77, 49, 116, 3)))
        assertNull(WatchProtocol.parse(byteArrayOf(77, 49, 108, 2)))
        assertNull(WatchProtocol.parse(byteArrayOf(77, 49, 118, 101)))
        assertNull(WatchProtocol.parse("M1Z".toByteArray()))
        assertNull(WatchProtocol.parse(byteArrayOf(77, 49, 113, 0)))
        assertNull(WatchProtocol.parse(byteArrayOf(77, 49, 106, 20)))
        assertNull(WatchProtocol.parse("M1j".toByteArray()))
        assertNull(WatchProtocol.parse(ByteArray(WatchProtocol.MAX_MESSAGE + 1) { 0x4D }))
    }

    @Test
    fun rgb565PacksTheTopBits() {
        assertEquals(0xF800, WatchProtocol.rgb565(0xFF0000))
        assertEquals(0x07E0, WatchProtocol.rgb565(0x00FF00))
        assertEquals(0x001F, WatchProtocol.rgb565(0x0000FF))
        assertEquals(0xFFFF, WatchProtocol.rgb565(0xFFFFFF))
    }
}
