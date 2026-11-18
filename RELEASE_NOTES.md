# Proton.js 0.2.0 Preview

Proton.js 0.2.0 is a standalone Servo-based runtime for HTML, CSS, and JavaScript applications. Proton Batcher (`pb`) creates a starter project and packages it for Windows, Linux, or Android.

## Downloads

- **Windows x64** includes `Proton.exe` and the standalone `pb.exe`. Extract the ZIP and open `Proton.exe` for the English getting-started screen.
- **Three.js demo for Windows x64** includes an offline 3D scene. Extract the full ZIP and open its `Proton.exe`. Keep the `web` directory beside the executable.
- **Android ARM64 debug APK** is a test build for Android 10 or later.
- **Source code** includes the Rust runtime, Proton Batcher, the Android template, and the Three.js example.

Use `SHA256SUMS.txt` to verify the downloadable files.

## Verification

- Windows x64 passed WebGL 1 and WebGL 2 pixel readback, WebGPU shader computation and readback, and the Three.js r187 first-frame test.
- The Android debug APK was built and its signature was verified. Runtime testing on an Android device is pending. WebGPU is unavailable from the default Android `file://` entry page.
- Linux packaging is available through `pb -build -linux` on a Linux host. Linux runtime and graphics verification are pending.

## Upstream components

The engine is based on [Servo 0.8.0](https://github.com/servo/servo/releases/tag/v0.8.0). See [Third-Party Notices](https://github.com/Digit-srt/proton.js/blob/v0.2.0/THIRD_PARTY_NOTICES.md) for the exact source revision and license information.
