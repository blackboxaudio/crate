package com.bbx_audio.crateapp

import android.content.Context
import com.google.android.gms.tasks.Tasks
import com.google.android.play.core.integrity.IntegrityManagerFactory
import com.google.android.play.core.integrity.IntegrityTokenRequest
import java.util.concurrent.TimeUnit

/**
 * Play Integrity bridge for Firebase App Check (#139).
 *
 * Called from Rust via JNI (see appcheck/play_integrity.rs). Mints a raw Play Integrity token for
 * a server-issued nonce; the Rust side then exchanges it for a Firebase App Check token via
 * `:exchangePlayIntegrityToken`. This **blocks** the calling thread (Rust invokes it on a
 * `spawn_blocking` task) and throws on failure, so the JNI layer surfaces a Java exception that
 * Rust maps to a `CrateError`.
 *
 * Requires the app to be linked in the Google Play Console and Play services on the device.
 */
object AppCheckPlayIntegrity {
    @JvmStatic
    fun requestToken(context: Context, cloudProjectNumber: Long, nonce: String): String {
        val manager = IntegrityManagerFactory.create(context.applicationContext)
        val request = IntegrityTokenRequest.builder()
            .setCloudProjectNumber(cloudProjectNumber)
            .setNonce(nonce)
            .build()
        // Block on the async request (the Rust caller runs us off the async runtime), bounded so a
        // hung Play Integrity call surfaces as a TimeoutException rather than stalling sync forever.
        val response = Tasks.await(manager.requestIntegrityToken(request), 20, TimeUnit.SECONDS)
        return response.token()
    }
}
