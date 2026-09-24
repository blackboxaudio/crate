package com.bbx_audio.crateapp

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import java.io.IOException
import java.security.KeyStore
import java.security.SecureRandom
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * SQLCipher key provisioning backed by the Android Keystore (#144).
 *
 * Called from Rust via JNI (see db/key_provider.rs). The Keystore cannot export raw key material,
 * so the standard wrapping pattern is used: a non-exportable AES-256-GCM key lives in the Keystore
 * and encrypts a random 64-char hex SQLCipher passphrase, whose wrapped form is persisted in
 * app-private SharedPreferences. The blob survives app updates and dies on uninstall — matching the
 * iOS Keychain provider's device-only, non-migrating semantics.
 *
 * IMPORTANT: if a wrapped blob exists but cannot be decrypted, this THROWS rather than generating a
 * fresh key — a new key would silently mismatch the already-encrypted database (same policy as the
 * iOS provider's locked-keychain branch). Rust maps the exception to `CrateError::KeyStorage`.
 *
 * Works from any `Context` (Activity or the headless WorkManager Worker), which is why the caller
 * passes one in rather than relying on process-global state.
 */
object CrateDbKey {
    private const val KEYSTORE_ALIAS = "crate_sqlcipher_wrap_v1"
    private const val PREFS_FILE = "crate_db_key"
    private const val PREFS_KEY_WRAPPED = "wrapped_key_v1"
    private const val GCM_TAG_BITS = 128
    private const val KEY_HEX_CHARS = 64

    @JvmStatic
    @Synchronized
    fun getOrCreateKey(context: Context): String {
        val prefs = context.applicationContext.getSharedPreferences(PREFS_FILE, Context.MODE_PRIVATE)

        val wrapped = prefs.getString(PREFS_KEY_WRAPPED, null)
        if (wrapped != null) {
            return unwrap(wrapped)
        }

        // First launch: generate a fresh passphrase, wrap it, and persist synchronously —
        // commit() (not apply()) so a crash right after can't lose the only copy of the key
        // for a database we're about to create.
        val hexKey = generateHexKey()
        val blob = wrap(hexKey)
        if (!prefs.edit().putString(PREFS_KEY_WRAPPED, blob).commit()) {
            throw IOException("failed to persist wrapped SQLCipher key")
        }
        return hexKey
    }

    /** 32 CSPRNG bytes as 64 lowercase hex chars — same format as the desktop/iOS providers. */
    private fun generateHexKey(): String {
        val bytes = ByteArray(KEY_HEX_CHARS / 2)
        SecureRandom().nextBytes(bytes)
        return bytes.joinToString("") { "%02x".format(it) }
    }

    /** Encrypt the passphrase with the Keystore key; returns `Base64(iv):Base64(ciphertext)`. */
    private fun wrap(hexKey: String): String {
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.ENCRYPT_MODE, getOrCreateWrappingKey())
        val ciphertext = cipher.doFinal(hexKey.toByteArray(Charsets.UTF_8))
        val iv = Base64.encodeToString(cipher.iv, Base64.NO_WRAP)
        val ct = Base64.encodeToString(ciphertext, Base64.NO_WRAP)
        return "$iv:$ct"
    }

    /** Decrypt a wrapped blob. Throws on any corruption or Keystore failure — never regenerates. */
    private fun unwrap(blob: String): String {
        val parts = blob.split(':')
        require(parts.size == 2) { "malformed wrapped key blob" }
        val iv = Base64.decode(parts[0], Base64.NO_WRAP)
        val ct = Base64.decode(parts[1], Base64.NO_WRAP)

        val keystore = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        val key = keystore.getKey(KEYSTORE_ALIAS, null) as? SecretKey
            ?: throw IllegalStateException("wrapped key blob exists but Keystore alias is missing")

        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.DECRYPT_MODE, key, GCMParameterSpec(GCM_TAG_BITS, iv))
        return String(cipher.doFinal(ct), Charsets.UTF_8)
    }

    /**
     * Load or create the non-exportable AES wrapping key. No user-auth constraint — the key must
     * be usable after first unlock without prompting (background sync opens the DB headlessly),
     * matching iOS's `kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly`.
     */
    private fun getOrCreateWrappingKey(): SecretKey {
        val keystore = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (keystore.getKey(KEYSTORE_ALIAS, null) as? SecretKey)?.let { return it }

        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
        generator.init(
            KeyGenParameterSpec.Builder(
                KEYSTORE_ALIAS,
                KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT
            )
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .build()
        )
        return generator.generateKey()
    }
}
