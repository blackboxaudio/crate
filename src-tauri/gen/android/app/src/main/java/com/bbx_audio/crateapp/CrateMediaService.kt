package com.bbx_audio.crateapp

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.media.AudioAttributes
import android.media.AudioFocusRequest
import android.media.AudioManager
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.support.v4.media.MediaMetadataCompat
import android.support.v4.media.session.MediaSessionCompat
import android.support.v4.media.session.PlaybackStateCompat
import androidx.core.app.NotificationCompat
import androidx.core.content.ContextCompat
import java.net.URL
import java.util.concurrent.Executors

/**
 * Foreground media service for discovery preview playback (#62).
 *
 * The audio itself plays in the WebView's HTML5 `<audio>` element — Android System WebView never
 * surfaces the W3C Media Session to the OS, so this service owns the OS-visible surface instead:
 * a MediaSessionCompat (lock screen / media carousel), a MediaStyle notification with transport
 * controls and seekbar, audio focus, and — critically — foreground-service status that keeps the
 * process (WebView renderer, Rust audio proxy) alive and unfrozen while playing in the background.
 *
 * Rust drives it through the @JvmStatic entry points (services/media_controls/android.rs); remote
 * commands and focus changes flow back through [nativeMediaEvent] → Tauri `media-*` events →
 * shared/services/androidMediaSession.ts, which applies them to the HTML5 player. All session and
 * notification work runs on the main looper (the statics are called from arbitrary Rust threads).
 */
class CrateMediaService : Service() {
    private var session: MediaSessionCompat? = null
    private var focusRequest: AudioFocusRequest? = null
    private var focusHeld = false
    private var ducked = false

    private val focusListener = AudioManager.OnAudioFocusChangeListener { change ->
        when (change) {
            AudioManager.AUDIOFOCUS_LOSS -> {
                focusHeld = false
                emit("focus_loss")
            }
            AudioManager.AUDIOFOCUS_LOSS_TRANSIENT -> emit("focus_loss_transient")
            AudioManager.AUDIOFOCUS_LOSS_TRANSIENT_CAN_DUCK -> {
                ducked = true
                emit("duck_start")
            }
            AudioManager.AUDIOFOCUS_GAIN -> {
                if (ducked) {
                    ducked = false
                    emit("duck_end")
                } else {
                    emit("focus_gain")
                }
            }
        }
    }

    /**
     * ACTION_AUDIO_BECOMING_NOISY: the audio route is about to fall back to the built-in speaker
     * because headphones were unplugged or a Bluetooth device disconnected. Android requires media
     * apps to pause here — otherwise the user's music suddenly blasts out of the phone speaker.
     * Mirrors the iOS route-change handling (`OldDeviceUnavailable` → pause).
     */
    private val becomingNoisyReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context?, intent: Intent?) {
            if (intent?.action == AudioManager.ACTION_AUDIO_BECOMING_NOISY) {
                emit("pause")
            }
        }
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        instance = this
        createChannel()
        ContextCompat.registerReceiver(
            this,
            becomingNoisyReceiver,
            IntentFilter(AudioManager.ACTION_AUDIO_BECOMING_NOISY),
            ContextCompat.RECEIVER_NOT_EXPORTED,
        )
        session = MediaSessionCompat(this, "CrateMediaService").apply {
            setCallback(object : MediaSessionCompat.Callback() {
                override fun onPlay() = emit("play")
                override fun onPause() = emit("pause")
                override fun onSkipToNext() = emit("next")
                override fun onSkipToPrevious() = emit("previous")
                override fun onSeekTo(pos: Long) = emit("seek", pos)
                override fun onStop() = emit("stop")
            })
            setSessionActivity(activityIntent())
        }
        applyMetadata()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_TOGGLE -> emit("toggle")
            ACTION_NEXT -> emit("next")
            ACTION_PREVIOUS -> emit("previous")
            else -> {
                // Initial start via startForegroundService: the 5s startForeground window applies.
                applyMetadata()
                applyPlayback()
            }
        }
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        instance = null
        runCatching { unregisterReceiver(becomingNoisyReceiver) }
        session?.release()
        session = null
        super.onDestroy()
    }

    /** Push companion state into the session metadata + notification; fetch artwork if new. */
    internal fun applyMetadata() {
        val s = session ?: return
        s.setMetadata(
            MediaMetadataCompat.Builder()
                .putString(MediaMetadataCompat.METADATA_KEY_TITLE, title)
                .putString(MediaMetadataCompat.METADATA_KEY_ARTIST, artist)
                .putString(MediaMetadataCompat.METADATA_KEY_ALBUM, album)
                .putLong(MediaMetadataCompat.METADATA_KEY_DURATION, durationMs)
                .putBitmap(MediaMetadataCompat.METADATA_KEY_ALBUM_ART, artBitmap)
                .build()
        )
        refreshNotification()
        fetchArtworkIfNeeded()
    }

    /** Push companion playback state; promote to / demote from foreground accordingly. */
    internal fun applyPlayback() {
        val s = session ?: return
        s.setPlaybackState(
            PlaybackStateCompat.Builder()
                .setActions(
                    PlaybackStateCompat.ACTION_PLAY or
                        PlaybackStateCompat.ACTION_PAUSE or
                        PlaybackStateCompat.ACTION_PLAY_PAUSE or
                        PlaybackStateCompat.ACTION_SKIP_TO_NEXT or
                        PlaybackStateCompat.ACTION_SKIP_TO_PREVIOUS or
                        PlaybackStateCompat.ACTION_SEEK_TO or
                        PlaybackStateCompat.ACTION_STOP
                )
                .setState(
                    if (playing) PlaybackStateCompat.STATE_PLAYING else PlaybackStateCompat.STATE_PAUSED,
                    positionMs,
                    if (playing) 1.0f else 0.0f,
                )
                .build()
        )
        s.isActive = true
        if (playing) {
            requestFocus()
            startForeground(NOTIFICATION_ID, buildNotification())
        } else {
            // Keep the (now dismissible) notification but drop foreground status — paused
            // playback shouldn't pin the process.
            refreshNotification()
            stopForeground(STOP_FOREGROUND_DETACH)
        }
    }

    /** Tear everything down (playback stopped / preview cleared). */
    internal fun shutdown() {
        abandonFocus()
        session?.isActive = false
        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf()
    }

    private fun requestFocus() {
        if (focusHeld) return
        val manager = getSystemService(Context.AUDIO_SERVICE) as AudioManager
        val request = focusRequest ?: AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN)
            .setAudioAttributes(
                AudioAttributes.Builder()
                    .setUsage(AudioAttributes.USAGE_MEDIA)
                    .setContentType(AudioAttributes.CONTENT_TYPE_MUSIC)
                    .build()
            )
            .setOnAudioFocusChangeListener(focusListener, Handler(Looper.getMainLooper()))
            .build()
            .also { focusRequest = it }
        focusHeld =
            manager.requestAudioFocus(request) == AudioManager.AUDIOFOCUS_REQUEST_GRANTED
    }

    private fun abandonFocus() {
        val request = focusRequest ?: return
        if (!focusHeld) return
        val manager = getSystemService(Context.AUDIO_SERVICE) as AudioManager
        manager.abandonAudioFocusRequest(request)
        focusHeld = false
        ducked = false
    }

    private fun createChannel() {
        val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        manager.createNotificationChannel(
            NotificationChannel(CHANNEL_ID, "Playback", NotificationManager.IMPORTANCE_LOW)
        )
    }

    private fun activityIntent(): PendingIntent = PendingIntent.getActivity(
        this,
        0,
        Intent(this, MainActivity::class.java),
        PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
    )

    private fun actionIntent(action: String, requestCode: Int): PendingIntent =
        PendingIntent.getService(
            this,
            requestCode,
            Intent(this, CrateMediaService::class.java).setAction(action),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )

    private fun buildNotification() = NotificationCompat.Builder(this, CHANNEL_ID)
        .setSmallIcon(R.mipmap.ic_launcher)
        .setLargeIcon(artBitmap)
        .setContentTitle(title ?: "")
        .setContentText(artist ?: "")
        .setSubText(album)
        .setContentIntent(activityIntent())
        .setVisibility(NotificationCompat.VISIBILITY_PUBLIC)
        .setOngoing(playing)
        .setOnlyAlertOnce(true)
        .setSilent(true)
        .addAction(android.R.drawable.ic_media_previous, "Previous", actionIntent(ACTION_PREVIOUS, 1))
        .addAction(
            if (playing) android.R.drawable.ic_media_pause else android.R.drawable.ic_media_play,
            if (playing) "Pause" else "Play",
            actionIntent(ACTION_TOGGLE, 2),
        )
        .addAction(android.R.drawable.ic_media_next, "Next", actionIntent(ACTION_NEXT, 3))
        .setStyle(
            androidx.media.app.NotificationCompat.MediaStyle()
                .setMediaSession(session?.sessionToken)
                .setShowActionsInCompactView(0, 1, 2)
        )
        .build()

    private fun refreshNotification() {
        val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        manager.notify(NOTIFICATION_ID, buildNotification())
    }

    /** Load album art off the main thread (http(s):// or file://), then re-apply metadata. */
    private fun fetchArtworkIfNeeded() {
        val url = artworkUrl
        if (url == fetchedArtworkUrl) return
        fetchedArtworkUrl = url
        if (url == null) {
            artBitmap = null
            return
        }
        artworkExecutor.execute {
            val bitmap = runCatching { loadBitmap(url) }.getOrNull()
            mainHandler.post {
                // Only apply if the artwork target hasn't moved on while we were fetching.
                if (url == fetchedArtworkUrl) {
                    artBitmap = bitmap
                    instance?.applyMetadata()
                }
            }
        }
    }

    private fun loadBitmap(url: String): Bitmap? = if (url.startsWith("file://")) {
        BitmapFactory.decodeFile(url.removePrefix("file://"))
    } else {
        URL(url).openStream().use { BitmapFactory.decodeStream(it) }
    }

    companion object {
        private const val CHANNEL_ID = "playback"
        private const val NOTIFICATION_ID = 6201
        private const val ACTION_TOGGLE = "audio.bbx.crate.media.TOGGLE"
        private const val ACTION_NEXT = "audio.bbx.crate.media.NEXT"
        private const val ACTION_PREVIOUS = "audio.bbx.crate.media.PREVIOUS"

        private val mainHandler = Handler(Looper.getMainLooper())
        private val artworkExecutor = Executors.newSingleThreadExecutor()
        private var instance: CrateMediaService? = null

        // Now-playing state pushed from Rust; lives in the companion so metadata that arrives
        // before the service starts isn't lost. Main-thread only (statics post to mainHandler).
        private var title: String? = null
        private var artist: String? = null
        private var album: String? = null
        private var artworkUrl: String? = null
        private var durationMs: Long = 0
        private var playing = false
        private var positionMs: Long = 0
        private var artBitmap: Bitmap? = null
        private var fetchedArtworkUrl: String? = null

        /**
         * Whether preview playback is currently active — read by [MainActivity.onPause] to decide
         * if the WebView must be kept alive while the activity backgrounds.
         */
        @Volatile
        var isPlaybackActive = false
            private set

        /**
         * Update the now-playing metadata (Rust → JNI). Safe to call from any thread. Metadata
         * alone never starts the service (the `context` parameter keeps the bridge signature
         * uniform); state is held here until [updatePlayback] brings the service up.
         */
        @JvmStatic
        fun updateMetadata(
            @Suppress("UNUSED_PARAMETER") context: Context,
            title: String?,
            artist: String?,
            album: String?,
            artworkUrl: String?,
            durationMs: Long,
        ) {
            mainHandler.post {
                this.title = title
                this.artist = artist
                this.album = album
                this.artworkUrl = artworkUrl
                this.durationMs = durationMs
                instance?.applyMetadata()
            }
        }

        /** Update play/pause + position (Rust → JNI). First play starts the foreground service. */
        @JvmStatic
        fun updatePlayback(context: Context, isPlaying: Boolean, positionMs: Long) {
            val appContext = context.applicationContext
            mainHandler.post {
                playing = isPlaying
                this.positionMs = positionMs
                isPlaybackActive = isPlaying
                val service = instance
                if (service != null) {
                    service.applyPlayback()
                } else if (isPlaying) {
                    // Always from a user gesture (play), so the FGS start is permitted.
                    ContextCompat.startForegroundService(
                        appContext,
                        Intent(appContext, CrateMediaService::class.java),
                    )
                }
            }
        }

        /** Tear down the session, notification, focus, and service (Rust → JNI). */
        @JvmStatic
        fun clear(@Suppress("UNUSED_PARAMETER") context: Context) {
            mainHandler.post {
                isPlaybackActive = false
                playing = false
                positionMs = 0
                title = null
                artist = null
                album = null
                artworkUrl = null
                fetchedArtworkUrl = null
                artBitmap = null
                durationMs = 0
                instance?.shutdown()
            }
        }

        // Implemented in Rust — services/media_controls/android.rs. `value` carries the seek
        // position (ms) and is 0 for every other event.
        @JvmStatic
        private external fun nativeMediaEvent(event: String, value: Long)

        internal fun emit(event: String, value: Long = 0) {
            try {
                nativeMediaEvent(event, value)
            } catch (t: Throwable) {
                // Never let a JNI failure take down the session callback / focus listener.
            }
        }

        init {
            // The cdylib name matches `[lib] name = "crate_lib"` in src-tauri/Cargo.toml. Wry has
            // always loaded it by the time Rust talks to us, but a notification action can class-
            // load us first — loading twice is a no-op.
            System.loadLibrary("crate_lib")
        }
    }
}
