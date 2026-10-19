package io.github.femboynyx.umbra

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import java.security.KeyStore
import java.security.SecureRandom
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * 32-byte SQLCipher key, wrapped by an Android Keystore AES-GCM key.
 * The wrapped blob lives in app-private preferences. The raw key is returned
 * once so Dart can pass it to `init` and then wipe it.
 */
class UmbraKeystore(private val context: Context) {
    fun getOrCreate(): Map<String, Any> {
        val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        val existing = prefs.getString(FIELD, null)
        val key = if (existing != null) unwrap(existing) else {
            val fresh = ByteArray(KEY_LEN).also { SecureRandom().nextBytes(it) }
            prefs.edit().putString(FIELD, wrap(fresh)).apply()
            fresh
        }
        return mapOf("bytes" to key, "backend" to "android-keystore")
    }

    private fun secretKey(): SecretKey {
        val store = KeyStore.getInstance(ANDROID_STORE).apply { load(null) }
        val existing = store.getKey(ALIAS, null) as? SecretKey
        if (existing != null) return existing
        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, ANDROID_STORE)
        generator.init(
            KeyGenParameterSpec.Builder(
                ALIAS,
                KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
            )
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setRandomizedEncryptionRequired(true)
                .build(),
        )
        return generator.generateKey()
    }

    private fun wrap(key: ByteArray): String {
        val cipher = Cipher.getInstance(TRANSFORM)
        cipher.init(Cipher.ENCRYPT_MODE, secretKey())
        val iv = cipher.iv
        val ciphertext = cipher.doFinal(key)
        val packed = ByteArray(iv.size + ciphertext.size)
        System.arraycopy(iv, 0, packed, 0, iv.size)
        System.arraycopy(ciphertext, 0, packed, iv.size, ciphertext.size)
        return Base64.encodeToString(packed, Base64.NO_WRAP)
    }

    private fun unwrap(encoded: String): ByteArray {
        val packed = Base64.decode(encoded, Base64.NO_WRAP)
        if (packed.size <= IV_LEN) error("stored db_key is truncated")
        val iv = packed.copyOfRange(0, IV_LEN)
        val ciphertext = packed.copyOfRange(IV_LEN, packed.size)
        val cipher = Cipher.getInstance(TRANSFORM)
        cipher.init(Cipher.DECRYPT_MODE, secretKey(), GCMParameterSpec(128, iv))
        val key = cipher.doFinal(ciphertext)
        if (key.size != KEY_LEN) error("stored db_key is not 32 bytes")
        return key
    }

    companion object {
        private const val ANDROID_STORE = "AndroidKeyStore"
        private const val ALIAS = "umbra_db_key_wrap"
        private const val TRANSFORM = "AES/GCM/NoPadding"
        private const val PREFS = "umbra_keystore"
        private const val FIELD = "wrapped_db_key"
        private const val IV_LEN = 12
        private const val KEY_LEN = 32
    }
}
