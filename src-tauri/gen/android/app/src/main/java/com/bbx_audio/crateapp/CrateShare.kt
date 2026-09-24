package com.bbx_audio.crateapp

import android.content.Context
import android.content.Intent

/**
 * Outbound share (the mirror of the inbound ShareIntentQueue): opens the system chooser for a
 * text/plain share. Called from Rust via JNI (`commands/share.rs` → `share_url`).
 */
object CrateShare {
    @JvmStatic
    fun share(context: Context, text: String, title: String?) {
        val send = Intent(Intent.ACTION_SEND).apply {
            type = "text/plain"
            putExtra(Intent.EXTRA_TEXT, text)
            if (title != null) putExtra(Intent.EXTRA_TITLE, title)
        }
        val chooser = Intent.createChooser(send, null).apply {
            // The adopted context may be the application context, not an Activity.
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        }
        context.startActivity(chooser)
    }
}
