import java.security.MessageDigest

plugins {
    id("com.android.application") version "9.4.1"
    id("org.jetbrains.kotlin.plugin.compose") version "2.4.21"
}

val servoAar = file(providers.environmentVariable("PROTON_SERVO_AAR").get())
val webAssets = file(providers.environmentVariable("PROTON_ASSETS_DIR").get())
val expectedDigest = "7d13e39fdba6a580d67c8e593ac71d147334c224420c984f1523657889008af3"

val verifyServo = tasks.register("verifyServo") {
    inputs.file(servoAar)
    doLast {
        val sha = MessageDigest.getInstance("SHA-256")
        servoAar.inputStream().buffered().use { stream ->
            val buffer = ByteArray(64 * 1024)
            var size = stream.read(buffer)
            while (size != -1) {
                sha.update(buffer, 0, size)
                size = stream.read(buffer)
            }
        }
        val actual = sha.digest().joinToString("") { "%02x".format(it.toInt() and 255) }
        check(actual == expectedDigest) {
            "Servo 0.8.0 SHA-256 mismatch for ${servoAar.path}"
        }
    }
}

android {
    namespace = "app.proton.runtime"
    compileSdk = 37
    buildToolsVersion = "36.0.0"
    defaultConfig {
        applicationId = providers.gradleProperty("protonAppId").get()
        minSdk = 29
        targetSdk = 37
        versionCode = 2
        versionName = "0.2.0"
        ndk { abiFilters += "arm64-v8a" }
    }
    buildFeatures { compose = true }
    sourceSets.getByName("main").assets.directories += webAssets.absolutePath
}

tasks.named("preBuild") { dependsOn(verifyServo) }

dependencies {
    implementation(files(servoAar))
    implementation("androidx.activity:activity-compose:1.13.0")
    implementation("androidx.compose.foundation:foundation:1.12.0")
}

