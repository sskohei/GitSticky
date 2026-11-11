package com.gitsticky.stickynote

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

@InvokeArg
class AuthWriteArgs { lateinit var value: String; var slot: String = "auth" }
@InvokeArg
class AuthReadArgs { var slot: String = "auth" }
@InvokeArg
class AuthUrlArgs { lateinit var url: String }

@TauriPlugin
class SecureAuthPlugin(private val activity: Activity) : Plugin(activity) {
    private fun checkedSlot(slot: String): String {
        check(slot in setOf("auth", "notes-key", "sync-keys"))
        return slot
    }
    private fun alias(slot: String) = "com.gitsticky.stickynote.github.${checkedSlot(slot)}.v1"
    private fun preferences(slot: String) = activity.getSharedPreferences(if (checkedSlot(slot) == "auth") "github_secure_auth" else "secure_$slot", Context.MODE_PRIVATE)
    private fun key(create: Boolean, slot: String): SecretKey {
        val alias = alias(slot)
        val preferences = preferences(slot)
        val store = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        val existing = store.getKey(alias, null)
        if (existing != null) return existing as SecretKey
        check(create && !preferences.contains("ciphertext"))
        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
        generator.init(KeyGenParameterSpec.Builder(alias, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM).setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setKeySize(256).build())
        return generator.generateKey()
    }
    @Command
    fun read(invoke: Invoke) {
        try {
            val slot = checkedSlot(invoke.parseArgs(AuthReadArgs::class.java).slot)
            val preferences = preferences(slot)
            val alias = alias(slot)
            val ciphertext = preferences.getString("ciphertext", null)
            if (ciphertext == null) { invoke.resolve(JSObject().put("value", org.json.JSONObject.NULL)); return }
            val iv = Base64.decode(preferences.getString("iv", null) ?: error("missing"), Base64.NO_WRAP)
            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            cipher.init(Cipher.DECRYPT_MODE, key(false, slot), GCMParameterSpec(128, iv))
            cipher.updateAAD(alias.toByteArray(Charsets.UTF_8))
            val plaintext = cipher.doFinal(Base64.decode(ciphertext, Base64.NO_WRAP))
            try { invoke.resolve(JSObject().put("value", String(plaintext, Charsets.UTF_8))) } finally { plaintext.fill(0) }
        } catch (_: Exception) { invoke.reject("端末の認証情報を復号できません。") }
    }
    @Command
    fun write(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(AuthWriteArgs::class.java)
            val slot = checkedSlot(args.slot)
            val preferences = preferences(slot)
            val alias = alias(slot)
            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            cipher.init(Cipher.ENCRYPT_MODE, key(true, slot))
            cipher.updateAAD(alias.toByteArray(Charsets.UTF_8))
            val plaintext = args.value.toByteArray(Charsets.UTF_8)
            val encrypted = try { cipher.doFinal(plaintext) } finally { plaintext.fill(0) }
            check(preferences.edit().putString("ciphertext", Base64.encodeToString(encrypted, Base64.NO_WRAP))
                .putString("iv", Base64.encodeToString(cipher.iv, Base64.NO_WRAP)).commit())
            invoke.resolve()
        } catch (_: Exception) { invoke.reject("端末の認証情報を保存できません。") }
    }
    @Command
    fun clear(invoke: Invoke) {
        try {
            val slot = checkedSlot(invoke.parseArgs(AuthReadArgs::class.java).slot)
            val preferences = preferences(slot)
            val alias = alias(slot)
            check(preferences.edit().clear().commit())
            val store = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
            if (store.containsAlias(alias)) store.deleteEntry(alias)
            invoke.resolve()
        }
        catch (_: Exception) { invoke.reject("認証情報を削除できません。") }
    }
    @Command
    fun openGithub(invoke: Invoke) {
        try {
            val uri = Uri.parse(invoke.parseArgs(AuthUrlArgs::class.java).url)
            check(uri.scheme == "https" && uri.host == "github.com")
            activity.startActivity(Intent(Intent.ACTION_VIEW, uri))
            invoke.resolve()
        } catch (_: Exception) { invoke.reject("GitHubをブラウザで開けません。") }
    }
}
