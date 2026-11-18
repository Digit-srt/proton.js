package app.proton.runtime

import android.app.AlertDialog
import android.net.Uri
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import org.servo.servoview.Servo
import org.servo.servoview.ServoNavigator
import java.io.File
import java.io.IOException

private const val GRAPHICS_ARGS = """["--pref", "dom_webgl2_enabled", "--pref", "dom_webgpu_enabled"]"""

class MainActivity : ComponentActivity(), Servo.Client {
    private val navigator = ServoNavigator()
    private var keyboardVisible by mutableStateOf(false)

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val entry = try {
            prepareWebAssets()
        } catch (error: IOException) {
            AlertDialog.Builder(this)
                .setTitle("Proton")
                .setMessage(error.localizedMessage)
                .setPositiveButton(android.R.string.ok) { _, _ -> finish() }
                .setOnCancelListener { finish() }
                .show()
            return
        }
        val url = Uri.fromFile(entry).toString()
        setContent {
            Servo(
                experimentalModeEnabled = false,
                softKeyboardVisible = keyboardVisible,
                args = GRAPHICS_ARGS,
                initialUrl = url,
                logStr = null,
                client = this,
                navigator = navigator,
                modifier = Modifier.fillMaxSize().safeDrawingPadding(),
            )
        }
    }

    private fun prepareWebAssets(): File {
        @Suppress("DEPRECATION")
        val updatedAt = packageManager.getPackageInfo(packageName, 0).lastUpdateTime
        val destination = File(noBackupFilesDir, "web-$updatedAt")
        val entry = File(destination, "index.html")
        if (entry.isFile) return entry

        val staging = File(noBackupFilesDir, "web-staging")
        staging.deleteRecursively()
        try {
            extractAssets("proton-web", staging)
            if (!File(staging, "index.html").isFile || !staging.renameTo(destination)) {
                throw IOException("Could not prepare the Proton web assets.")
            }
        } finally {
            staging.deleteRecursively()
        }
        noBackupFilesDir.listFiles()
            ?.filter { it.name.startsWith("web-") && it != destination }
            ?.forEach { it.deleteRecursively() }
        return entry
    }

    private fun extractAssets(path: String, destination: File) {
        val children = assets.list(path).orEmpty()
        if (children.isEmpty()) {
            assets.open(path).use { source -> destination.outputStream().use { source.copyTo(it) } }
            return
        }
        if (!destination.mkdir()) throw IOException("Could not create directory ${destination.name}")
        for (child in children) {
            extractAssets("$path/$child", File(destination, child))
        }
    }

    override fun onAlert(message: String) {
        AlertDialog.Builder(this).setMessage(message).setPositiveButton(android.R.string.ok, null).show()
    }

    override fun onLoadStarted() = Unit
    override fun onLoadEnded() = Unit
    override fun onTitleChanged(title: String) = Unit
    override fun onUrlChanged(url: String) = Unit
    override fun onImeShow() { keyboardVisible = true }
    override fun onImeHide() { keyboardVisible = false }
}
