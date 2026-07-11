package com.bbx_audio.crateapp

import org.json.JSONArray

/**
 * Pending share-intent texts (#62), queued until the frontend drains them.
 *
 * `ACTION_SEND` shares land in [MainActivity] — on a cold start that's `onCreate`, long before
 * the webview (or the Rust library) is ready to receive anything, so delivery is pull-based:
 * MainActivity enqueues here, and the frontend drains via the `take_shared_texts` Tauri command
 * (Rust calls [takeAll] over JNI — see commands/share.rs) on boot, on the warm-share nudge
 * event, and on foregrounding.
 */
object ShareIntentQueue {
    private val pending = mutableListOf<String>()

    @Synchronized
    fun enqueue(text: String) {
        pending.add(text)
    }

    /** Drain the queue as a JSON string array (called from Rust via JNI). */
    @JvmStatic
    @Synchronized
    fun takeAll(): String {
        val out = JSONArray(pending).toString()
        pending.clear()
        return out
    }
}
