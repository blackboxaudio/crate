package com.bbx_audio.crateapp

import android.content.Context
import androidx.work.Constraints
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.NetworkType
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import java.util.concurrent.TimeUnit

/**
 * WorkManager scheduling for opportunistic background cloud sync (#61).
 *
 * Called from Rust via JNI (services/cloud_sync/background/android.rs) on the foreground command
 * path — after sign-in (schedule) and on sign-out (cancel). Periodic work is floored at 15 minutes
 * and the OS decides real cadence; the CONNECTED constraint keeps it opportunistic (online only).
 * KEEP policy means relaunching the app doesn't reset an already-scheduled cycle.
 */
object CrateSyncScheduler {
    private const val WORK_NAME = "crate-cloud-sync"

    @JvmStatic
    fun schedule(context: Context) {
        val constraints = Constraints.Builder()
            .setRequiredNetworkType(NetworkType.CONNECTED)
            .build()
        val request = PeriodicWorkRequestBuilder<CrateSyncWorker>(15, TimeUnit.MINUTES)
            .setConstraints(constraints)
            .build()
        WorkManager.getInstance(context.applicationContext)
            .enqueueUniquePeriodicWork(WORK_NAME, ExistingPeriodicWorkPolicy.KEEP, request)
    }

    @JvmStatic
    fun cancel(context: Context) {
        WorkManager.getInstance(context.applicationContext).cancelUniqueWork(WORK_NAME)
    }
}
