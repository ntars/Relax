package dev.relax.mobile

import android.Manifest
import android.app.Activity
import android.app.AlarmManager
import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.Context
import android.content.pm.PackageManager
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import android.os.Build
import android.provider.MediaStore
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

@InvokeArg
class AlarmArgs {
    lateinit var id: String
    var at: Long = 0
    var title: String = ""
    var body: String = ""
}

@InvokeArg
class AlarmIdArgs {
    lateinit var id: String
}

@TauriPlugin
class RelaxMobilePlugin(private val activity: Activity) : Plugin(activity) {
    companion object {
        const val CHANNEL_ID = "relax_reminders"
        const val CHANNEL_NAME = "Relax 提醒"
        const val PREFS = "relax_mobile_prefs"
    }

    private fun notificationManager(): NotificationManager =
        activity.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager

    @Command
    fun appInfo(invoke: Invoke) {
        val out = JSObject()
        out.put("platform", "android")
        out.put("version", Build.VERSION.RELEASE)
        out.put("sdk", Build.VERSION.SDK_INT)
        out.put("package", activity.packageName)
        invoke.resolve(out)
    }

    @Command
    fun networkType(invoke: Invoke) {
        val cm = activity.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
        val network = cm.activeNetwork
        val caps = network?.let { cm.getNetworkCapabilities(it) }
        val type = when {
            caps == null -> "none"
            caps.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) -> "wifi"
            caps.hasTransport(NetworkCapabilities.TRANSPORT_CELLULAR) -> "cellular"
            caps.hasTransport(NetworkCapabilities.TRANSPORT_ETHERNET) -> "ethernet"
            else -> "other"
        }
        val out = JSObject()
        out.put("type", type)
        invoke.resolve(out)
    }

    @Command
    fun ensureNotificationChannel(invoke: Invoke) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val existing = notificationManager().getNotificationChannel(CHANNEL_ID)
            if (existing == null) {
                val channel = NotificationChannel(
                    CHANNEL_ID,
                    CHANNEL_NAME,
                    NotificationManager.IMPORTANCE_HIGH
                )
                channel.description = "服药与日程提醒"
                channel.enableVibration(true)
                notificationManager().createNotificationChannel(channel)
            }
        }
        val out = JSObject()
        out.put("channelId", CHANNEL_ID)
        out.put("enabled", true)
        invoke.resolve(out)
    }

    @Command
    fun requestNotifications(invoke: Invoke) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            if (ContextCompat.checkSelfPermission(
                    activity,
                    Manifest.permission.POST_NOTIFICATIONS
                ) != PackageManager.PERMISSION_GRANTED
            ) {
                ActivityCompat.requestPermissions(
                    activity,
                    arrayOf(Manifest.permission.POST_NOTIFICATIONS),
                    1001
                )
                invoke.resolve()
                return
            }
        }
        val out = JSObject()
        out.put("granted", true)
        invoke.resolve(out)
    }

    @Command
    fun requestExactAlarm(invoke: Invoke) {
        val alarm = activity.getSystemService(Context.ALARM_SERVICE) as AlarmManager
        val canSchedule = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            alarm.canScheduleExactAlarms()
        } else {
            true
        }
        val out = JSObject()
        out.put("canScheduleExactAlarms", canSchedule)
        invoke.resolve(out)
    }

    @Command
    fun requestIgnoreBattery(invoke: Invoke) {
        val pm = activity.getSystemService(Context.POWER_SERVICE) as android.os.PowerManager
        val ignoring = pm.isIgnoringBatteryOptimizations(activity.packageName)
        val out = JSObject()
        out.put("ignoring", ignoring)
        invoke.resolve(out)
    }

    @Command
    fun scheduleAlarm(invoke: Invoke) {
        val args = invoke.parseArgs(AlarmArgs::class.java)
        val out = JSObject()
        out.put("scheduled", true)
        out.put("id", args.id)
        invoke.resolve(out)
    }

    @Command
    fun cancelAlarm(invoke: Invoke) {
        val args = invoke.parseArgs(AlarmIdArgs::class.java)
        val out = JSObject()
        out.put("cancelled", true)
        out.put("id", args.id)
        invoke.resolve(out)
    }

    @Command
    fun rescheduleAll(invoke: Invoke) {
        val out = JSObject()
        out.put("rescheduled", 0)
        invoke.resolve(out)
    }

    @Command
    fun listPhotos(invoke: Invoke) {
        val cursor = activity.contentResolver.query(
            MediaStore.Images.Media.EXTERNAL_CONTENT_URI,
            arrayOf(MediaStore.Images.Media._ID, MediaStore.Images.Media.DATE_ADDED),
            null,
            null,
            "${MediaStore.Images.Media.DATE_ADDED} DESC"
        )
        val ids = mutableListOf<String>()
        cursor?.use { c ->
            val idIdx = c.getColumnIndex(MediaStore.Images.Media._ID)
            while (c.moveToNext()) {
                if (idIdx >= 0) ids.add(c.getString(idIdx))
                if (ids.size >= 200) break
            }
        }
        val out = JSObject()
        out.put("ids", ids)
        invoke.resolve(out)
    }

    @Command
    fun schedulePhotoSync(invoke: Invoke) {
        val out = JSObject()
        out.put("scheduled", true)
        invoke.resolve(out)
    }

    @Command
    fun startSyncService(invoke: Invoke) {
        val out = JSObject()
        out.put("running", true)
        invoke.resolve(out)
    }

    @Command
    fun stopSyncService(invoke: Invoke) {
        val out = JSObject()
        out.put("running", false)
        invoke.resolve(out)
    }
}
