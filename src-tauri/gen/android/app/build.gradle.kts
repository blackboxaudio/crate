import java.util.Properties

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("rust")
}

val tauriProperties = Properties().apply {
    val propFile = file("tauri.properties")
    if (propFile.exists()) {
        propFile.inputStream().use { load(it) }
    }
}

// Release signing — loaded from key.properties when present (CI writes this file from secrets).
// Without key.properties the release build stays unsigned; Gradle configuration still succeeds.
// Preserve this block if `tauri android init` regenerates the file.
val keystoreProperties = Properties()
val keystorePropertiesFile = file("key.properties")
if (keystorePropertiesFile.exists()) {
    keystorePropertiesFile.inputStream().use { keystoreProperties.load(it) }
}

// Channel identity (application id / label / OAuth redirect scheme) — stamped into the gitignored
// channel.properties by `scripts/write-android-channel.mjs --channel <dev|staging|prod>`. When the
// file is absent (fresh clone, bare Android Studio build) the getProperty defaults below make it
// the dev channel. Only `applicationId` varies per channel — `namespace` stays
// `com.bbx_audio.crateapp` because the Kotlin package and the hand-written Rust JNI symbols
// (CrateSyncWorker, CrateDbKey, …) are literal against it.
// Preserve this block if `tauri android init` regenerates the file.
val channelProperties = Properties()
val channelPropertiesFile = file("channel.properties")
if (channelPropertiesFile.exists()) {
    channelPropertiesFile.inputStream().use { channelProperties.load(it) }
}

android {
    compileSdk = 36
    namespace = "com.bbx_audio.crateapp"

    if (keystorePropertiesFile.exists()) {
        signingConfigs {
            create("release") {
                storeFile = file(keystoreProperties.getProperty("storeFile"))
                storePassword = keystoreProperties.getProperty("storePassword")
                keyAlias = keystoreProperties.getProperty("keyAlias")
                keyPassword = keystoreProperties.getProperty("keyPassword")
            }
        }
    }

    defaultConfig {
        // Cleartext policy actually lives in res/xml/network_security_config.xml (release scopes
        // it to the localhost audio proxy; the debug source set opens it up for the LAN dev
        // server). The placeholder is kept for parity with the tauri template manifest.
        manifestPlaceholders["usesCleartextTraffic"] = "false"
        manifestPlaceholders["oauthRedirectScheme"] =
            channelProperties.getProperty("oauthRedirectScheme", "crate.oauth.unset")
        applicationId = channelProperties.getProperty("applicationId", "com.bbx_audio.crate.dev")
        // Per-channel display name via resValue — the same-named keys were removed from
        // res/values/strings.xml (Gradle resValue + strings.xml would be a duplicate resource).
        resValue("string", "app_name", channelProperties.getProperty("appName", "Crate Dev"))
        resValue("string", "main_activity_title", channelProperties.getProperty("appName", "Crate Dev"))
        minSdk = 29
        targetSdk = 36
        versionCode = tauriProperties.getProperty("tauri.android.versionCode", "1").toInt()
        versionName = tauriProperties.getProperty("tauri.android.versionName", "1.0")
    }
    buildTypes {
        getByName("debug") {
            manifestPlaceholders["usesCleartextTraffic"] = "true"
            isDebuggable = true
            isJniDebuggable = true
            isMinifyEnabled = false
            packaging {                jniLibs.keepDebugSymbols.add("*/arm64-v8a/*.so")
                jniLibs.keepDebugSymbols.add("*/armeabi-v7a/*.so")
                jniLibs.keepDebugSymbols.add("*/x86/*.so")
                jniLibs.keepDebugSymbols.add("*/x86_64/*.so")
            }
        }
        getByName("release") {
            isMinifyEnabled = true
            if (keystorePropertiesFile.exists()) {
                signingConfig = signingConfigs.getByName("release")
            }
            proguardFiles(
                *fileTree(".") { include("**/*.pro") }
                    .plus(getDefaultProguardFile("proguard-android-optimize.txt"))
                    .toList().toTypedArray()
            )
        }
    }
    kotlinOptions {
        jvmTarget = "1.8"
    }
    buildFeatures {
        buildConfig = true
    }
}

rust {
    rootDirRel = "../../../"
}

dependencies {
    implementation("androidx.webkit:webkit:1.14.0")
    implementation("androidx.appcompat:appcompat:1.7.1")
    implementation("androidx.activity:activity-ktx:1.10.1")
    implementation("com.google.android.material:material:1.12.0")
    // App Check via Play Integrity (#139): mints the device/app integrity token that the Rust
    // App Check exchange (appcheck/play_integrity.rs) trades for a Firebase App Check token.
    implementation("com.google.android.play:integrity:1.4.0")
    // WorkManager backs opportunistic background cloud sync (#61): CrateSyncScheduler enqueues a
    // periodic CrateSyncWorker that calls into Rust via JNI (background/android.rs).
    implementation("androidx.work:work-runtime-ktx:2.9.1")
    // MediaSessionCompat + MediaStyle notification for background preview playback (#62):
    // CrateMediaService owns the lock-screen/notification surface, driven from Rust
    // (media_controls/android.rs) since the WebView doesn't surface the Web Media Session API.
    implementation("androidx.media:media:1.7.0")
    testImplementation("junit:junit:4.13.2")
    androidTestImplementation("androidx.test.ext:junit:1.1.4")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.5.0")
}

apply(from = "tauri.build.gradle.kts")