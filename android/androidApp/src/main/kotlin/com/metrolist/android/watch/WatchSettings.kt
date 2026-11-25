/*
 * Metrolist Project (C) 2026
 * Licensed under GPL-3.0 | See git history for contributors
 */

package com.metrolist.android.watch

import android.content.Context
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update

/**
 * The Settings → Watch switch and the address of the watch it last found.
 *
 * The switch is off by default: while it is off the fork never holds a GATT link.
 */
internal object WatchSettings {
    private const val FILE = "metrolist_watch_link"
    private const val KEY_ENABLED = "enabled"
    private const val KEY_ADDRESS = "address"

    private fun prefs(context: Context) = context.applicationContext.getSharedPreferences(FILE, Context.MODE_PRIVATE)

    fun enabled(context: Context): Boolean = prefs(context).getBoolean(KEY_ENABLED, false)

    fun setEnabled(
        context: Context,
        enabled: Boolean,
    ) {
        prefs(context).edit().putBoolean(KEY_ENABLED, enabled).apply()
    }

    fun address(context: Context): String? = prefs(context).getString(KEY_ADDRESS, null)?.takeIf { it.isNotBlank() }

    fun setAddress(
        context: Context,
        address: String,
    ) {
        prefs(context).edit().putString(KEY_ADDRESS, address).apply()
    }
}

internal enum class WatchLinkPhase {
    OFF,
    CONNECTING,
    CONNECTED,
    WAITING_FOR_WATCH,
    ERROR,
    NO_PERMISSION,
}

internal data class WatchLinkStatus(
    val phase: WatchLinkPhase = WatchLinkPhase.OFF,
    val deviceName: String? = null,
    /** A short technical reason for the Settings screen; never a frame, a key or a track URL. */
    val detail: String? = null,
    val lastFrameAt: Long = 0L,
    /** The bundled watch app's install progress, shown only on Settings → Watch. */
    val installMessage: String? = null,
    val installPercent: Int? = null,
)

/** The link state the Settings screen and the service share. */
internal object WatchLinkState {
    private val _status = MutableStateFlow(WatchLinkStatus())

    val status: StateFlow<WatchLinkStatus> = _status.asStateFlow()

    fun update(block: (WatchLinkStatus) -> WatchLinkStatus) {
        _status.update(block)
    }

    fun reset() {
        _status.value = WatchLinkStatus()
    }
}
