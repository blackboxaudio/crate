package com.bbx_audio.crateapp

import android.content.Context
import androidx.work.CoroutineWorker
import androidx.work.WorkerParameters
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * WorkManager worker for opportunistic background cloud sync (#61).
 *
 * Runs the native Rust sync via JNI. WorkManager may cold-start the process WITHOUT the Tauri
 * Activity, so this loads the native library explicitly (the Activity path loads it via wry) and
 * passes its app Context + data dir into Rust, which rebuilds the sync dependencies headlessly
 * (see services/cloud_sync/background/android.rs).
 *
 * Returns retry() on any failure so WorkManager backs off and tries again — offline, or (until
 * #144) before the Android Keystore DB-key provider can open the encrypted database.
 */
class CrateSyncWorker(appContext: Context, params: WorkerParameters) :
    CoroutineWorker(appContext, params) {

    override suspend fun doWork(): Result = withContext(Dispatchers.IO) {
        try {
            // filesDir MUST resolve to the same directory the app's setup() uses for crate.db
            // (Tauri `app_data_dir`), or a second empty DB opens and the sync silently no-ops.
            val filesDir = applicationContext.filesDir.absolutePath
            if (runBackgroundSync(applicationContext, filesDir)) Result.success() else Result.retry()
        } catch (t: Throwable) {
            Result.retry()
        }
    }

    // Implemented in Rust — services/cloud_sync/background/android.rs.
    private external fun runBackgroundSync(context: Context, filesDir: String): Boolean

    companion object {
        init {
            // The cdylib name matches `[lib] name = "crate_lib"` in src-tauri/Cargo.toml.
            System.loadLibrary("crate_lib")
        }
    }
}
