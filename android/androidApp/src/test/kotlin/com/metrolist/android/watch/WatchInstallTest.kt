/*
 * Metrolist Project (C) 2026
 * Licensed under GPL-3.0 | See git history for contributors
 */

package com.metrolist.android.watch

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.nio.charset.StandardCharsets
import java.util.zip.CRC32

class WatchInstallTest {
    private fun put32(bytes: ByteArray, at: Int, value: Long) {
        bytes[at] = value.toByte()
        bytes[at + 1] = (value shr 8).toByte()
        bytes[at + 2] = (value shr 16).toByte()
        bytes[at + 3] = (value shr 24).toByte()
    }

    private fun header(name: String = "metrolist", kind: Int = WatchInstall.KIND_APP): ByteArray {
        val raw = ByteArray(WatchInstall.HEADER_SIZE)
        "WP2A".toByteArray(StandardCharsets.US_ASCII).copyInto(raw, 0)
        put32(raw, 4, 1)
        raw[6] = kind.toByte()
        raw[7] = 3
        name.toByteArray(StandardCharsets.UTF_8).copyInto(raw, 8)
        put32(raw, 32, 0x00000C00L)
        put32(raw, 36, 100)
        put32(raw, 40, 40)
        put32(raw, 44, 2)
        put32(raw, 52, CRC32().apply { update(raw, 0, 52) }.value)
        return raw
    }

    private fun packageBytes(): ByteArray {
        val raw = header()
        val body = ByteArray(108)
        put32(raw, 48, CRC32().apply { update(body) }.value)
        put32(raw, 52, CRC32().apply { update(raw, 0, 52) }.value)
        return raw + body
    }

    @Test
    fun parsesAndValidatesTheSamePackageAsThePcTool() {
        val parsed = WatchInstall.parsePackage(packageBytes())
        assertEquals("metrolist", parsed.name)
        assertEquals(WatchInstall.KIND_APP, parsed.kind)
        assertEquals(1, parsed.abi)
        assertEquals(3, parsed.abiMinor)
        assertEquals("0.3.0", WatchInstall.versionText(0x00000C00))

        val badBody = packageBytes().also { it[70] = (it[70] + 1).toByte() }
        assertTrue(runCatching { WatchInstall.parsePackage(badBody) }.exceptionOrNull().toString().contains("image CRC"))
    }

    @Test
    fun slotChoiceAndChunkingMatchInstallDotPy() {
        val used = mapOf(0 to "other", 1 to "metrolist", 3 to "third", 255 to "loader")
        assertEquals(1, WatchInstall.chooseSlot("metrolist", used))
        assertEquals(2, WatchInstall.chooseSlot("new", used))
        assertEquals(380, WatchInstall.chunkCount(48561))
        val value = 0xDEADBEEFL
        assertEquals(value, WatchInstall.le32(WatchInstall.u32le(value), 0))
    }

    @Test
    fun refusesLoaderPackagesAndReadableStatusReplies() {
        assertEquals(WatchInstall.KIND_LOADER, WatchInstall.parseHeader(header(kind = WatchInstall.KIND_LOADER)).kind)
        assertTrue(WatchInstall.statusName(WatchInstall.STATUS_BUSY).contains("busy"))
        assertEquals(3, WatchInstall.stubAbiMinor(byteArrayOf(0, 1, 0, 0, 0, 3)))
        assertNull(WatchInstall.stubAbiMinor(byteArrayOf(0, 1)))
    }
}
