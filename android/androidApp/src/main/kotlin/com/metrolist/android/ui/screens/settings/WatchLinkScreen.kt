/*
 * Metrolist Project (C) 2026
 * Licensed under GPL-3.0 | See git history for contributors
 */

package com.metrolist.android.ui.screens.settings

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import android.widget.Toast
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import androidx.core.content.ContextCompat
import com.metrolist.android.watch.WatchLinkPhase
import com.metrolist.android.watch.WatchLinkService
import com.metrolist.android.watch.WatchLinkState
import com.metrolist.android.watch.WatchLinkStatus
import com.metrolist.android.watch.WatchSettings
import com.metrolist.shared.generated.resources.Res
import com.metrolist.shared.generated.resources.watch_link_connect
import com.metrolist.shared.generated.resources.watch_link_connected
import com.metrolist.shared.generated.resources.watch_link_connecting
import com.metrolist.shared.generated.resources.watch_link_firmware_note
import com.metrolist.shared.generated.resources.watch_link_install
import com.metrolist.shared.generated.resources.watch_link_install_desc
import com.metrolist.shared.generated.resources.watch_link_install_off
import com.metrolist.shared.generated.resources.watch_link_no_permission
import com.metrolist.shared.generated.resources.watch_link_off
import com.metrolist.shared.generated.resources.watch_link_pair_hint
import com.metrolist.shared.generated.resources.watch_link_test
import com.metrolist.shared.generated.resources.watch_link_title
import com.metrolist.shared.generated.resources.watch_link_waiting
import org.jetbrains.compose.resources.stringResource

/**
 * Settings → Watch: the switch, the connection state and a test action.
 *
 * Nothing here shows the channel key, a raw frame or a track URL; the detail line is the service's short reason only.
 */
@Composable
fun WatchLinkScreen(onNavigateBack: () -> Unit) {
    val context = LocalContext.current
    val status by WatchLinkState.status.collectAsState()
    var enabled by remember { mutableStateOf(WatchSettings.enabled(context)) }

    LaunchedEffect(status.phase) {
        if (status.phase == WatchLinkPhase.OFF) enabled = WatchSettings.enabled(context)
    }

    val permission =
        rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
            if (granted) {
                WatchLinkService.start(context)
                enabled = true
            } else {
                Toast.makeText(context, "Bluetooth permission is needed for the watch link", Toast.LENGTH_LONG).show()
            }
        }

    fun enableLink() {
        val needsPermission =
            Build.VERSION.SDK_INT >= Build.VERSION_CODES.S &&
                ContextCompat.checkSelfPermission(context, Manifest.permission.BLUETOOTH_CONNECT) !=
                PackageManager.PERMISSION_GRANTED
        if (needsPermission) {
            permission.launch(Manifest.permission.BLUETOOTH_CONNECT)
        } else {
            WatchLinkService.start(context)
            enabled = true
        }
    }

    fun disableLink() {
        WatchLinkService.stop(context)
        enabled = false
    }

    val installDescription =
        when {
            !enabled -> stringResource(Res.string.watch_link_install_off)
            status.installMessage != null ->
                status.installPercent?.let { "${status.installMessage} · $it%" } ?: status.installMessage
            else -> stringResource(Res.string.watch_link_install_desc)
        }
    val installing =
        status.installMessage?.let { message ->
            !message.startsWith("Installed") && !message.startsWith("Install failed")
        } == true

    SettingsSubScreen(
        title = stringResource(Res.string.watch_link_title),
        onNavigateBack = onNavigateBack,
    ) {
        SettingsSubScreenGroup(
            items =
                listOf(
                    switchSettingsItem(
                        title = stringResource(Res.string.watch_link_connect),
                        description = statusText(status),
                        checked = enabled,
                        onCheckedChange = { on -> if (on) enableLink() else disableLink() },
                    ),
                    SettingsSubScreenItem(
                        title = stringResource(Res.string.watch_link_test),
                        description = status.detail,
                        enabled = enabled,
                        onClick = { WatchLinkService.reconnect(context) },
                    ),
                    SettingsSubScreenItem(
                        title = stringResource(Res.string.watch_link_install),
                        description = installDescription,
                        enabled = enabled && !installing,
                        onClick = { WatchLinkService.install(context) },
                    ),
                    SettingsSubScreenItem(
                        title = stringResource(Res.string.watch_link_pair_hint),
                        description = stringResource(Res.string.watch_link_firmware_note),
                    ),
                ),
        )
    }
}

@Composable
private fun statusText(status: WatchLinkStatus): String =
    when (status.phase) {
        WatchLinkPhase.OFF -> stringResource(Res.string.watch_link_off)
        WatchLinkPhase.CONNECTING -> stringResource(Res.string.watch_link_connecting)
        WatchLinkPhase.CONNECTED -> stringResource(Res.string.watch_link_connected)
        WatchLinkPhase.WAITING_FOR_WATCH -> stringResource(Res.string.watch_link_waiting)
        WatchLinkPhase.ERROR -> status.detail ?: stringResource(Res.string.watch_link_off)
        WatchLinkPhase.NO_PERMISSION -> stringResource(Res.string.watch_link_no_permission)
    }
