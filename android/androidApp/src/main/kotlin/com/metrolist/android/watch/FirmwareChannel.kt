/*
 * Metrolist Project (C) 2026
 * Licensed under GPL-3.0 | See git history for contributors
 */

package com.metrolist.android.watch

import java.io.ByteArrayOutputStream
import java.util.zip.CRC32
import javax.crypto.Cipher
import javax.crypto.spec.IvParameterSpec
import javax.crypto.spec.SecretKeySpec

/**
 * The patched firmware's app channel (cmf-watch-sdk `docs/abi-1.4.md`, "Writing a host").
 *
 * A frame is `F5`, `length:u16be`, `0xFFFF`, `00 01`, `00 01`, `opcode:u16be`, then an AES-128-CBC body
 * (fixed IV, PKCS7, CRC32 of the plaintext appended). This is the same wire format the watch apps use; do not
 * change it here alone.
 */
internal object FirmwareChannel {
    const val INFO = 0x90A0
    const val BEGIN = 0x90A1
    const val DATA = 0x90A2
    const val END = 0x90A3
    const val MSG = 0x90A5
    const val PING = 0x90A7
    const val EVENT = 0xB0A6

    const val STATUS_OK = 0
    const val STATUS_NO_APP = 7
    const val STATUS_FULL = 8

    private val IV = hex("5051525354555657606162636465665A")

    private fun hex(s: String) = ByteArray(s.length / 2) { s.substring(2 * it, 2 * it + 2).toInt(16).toByte() }

    private fun crc(data: ByteArray): ByteArray {
        val v = CRC32().apply { update(data) }.value
        return byteArrayOf(v.toByte(), (v shr 8).toByte(), (v shr 16).toByte(), (v shr 24).toByte())
    }

    private fun cipher(
        mode: Int,
        key: ByteArray,
    ) = Cipher.getInstance("AES/CBC/PKCS5Padding").apply {
        init(mode, SecretKeySpec(key, "AES"), IvParameterSpec(IV))
    }

    fun seal(
        key: ByteArray,
        plain: ByteArray,
    ): ByteArray = cipher(Cipher.ENCRYPT_MODE, key).doFinal(plain + crc(plain))

    /** The plaintext, or `null` when the body was not sealed with this key (another host's frame). */
    fun open(
        key: ByteArray,
        body: ByteArray,
    ): ByteArray? =
        runCatching {
            val padded = cipher(Cipher.DECRYPT_MODE, key).doFinal(body)
            val plain = padded.copyOfRange(0, padded.size - 4)
            plain.takeIf { crc(it).contentEquals(padded.copyOfRange(padded.size - 4, padded.size)) }
        }.getOrNull()

    fun frame(
        opcode: Int,
        plain: ByteArray,
        key: ByteArray,
    ): ByteArray {
        val body = seal(key, plain)
        val out = ByteArrayOutputStream(11 + body.size)
        fun u16(v: Int) {
            out.write(v shr 8 and 255)
            out.write(v and 255)
        }
        out.write(0xF5)
        u16(body.size)
        u16(0xFFFF)
        u16(1)
        u16(1)
        u16(opcode)
        out.write(body)
        return out.toByteArray()
    }

    data class Packet(
        val c1: Int,
        val c2: Int,
        val body: ByteArray,
    )

    /**
     * The app's message out of an `EVENT` plaintext.
     *
     * An `EVENT` body is not the message itself: `docs/abi-1.1.md`, "Data channel" gives it as
     * `seq: u8 | slot: u8 | data[1..=128]`, and the reference host strips the same two bytes
     * (`wp2apps.channel.message`: `Message(plain[0], plain[1], plain[2:])`). Passing the envelope
     * straight to the `M1` parser made every watch frame look like it started with `seq`/`slot`
     * instead of `M1`, so all of them were dropped.
     */
    fun appData(plain: ByteArray): ByteArray? {
        if (plain.size < 3) return null
        return plain.copyOfRange(2, plain.size)
    }

    /** Reassembles frames that span several notifications. */
    class Parser {
        private var buf = ByteArray(0)

        fun feed(incoming: ByteArray): List<Packet> {
            buf += incoming
            val result = ArrayList<Packet>()
            while (true) {
                val start = buf.indexOfFirst { it.toInt() and 255 == 0xF5 }
                if (start < 0) {
                    buf = ByteArray(0)
                    break
                }
                if (start > 0) buf = buf.copyOfRange(start, buf.size)
                if (buf.size < 11) break
                fun u16(i: Int) = (buf[i].toInt() and 255) shl 8 or (buf[i + 1].toInt() and 255)
                val size = u16(1)
                if (size !in 1..4096) {
                    buf = buf.copyOfRange(1, buf.size)
                    continue
                }
                if (buf.size < 11 + size) break
                result += Packet(u16(3), u16(9), buf.copyOfRange(11, 11 + size))
                buf = buf.copyOfRange(11 + size, buf.size)
            }
            return result
        }
    }
}
