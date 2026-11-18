use std::env;
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{self, Command};

const DESKTOP_MANIFEST: &str = include_str!("../runtime/desktop/Cargo.toml");
const DESKTOP_LOCK: &str = include_str!("../runtime/desktop/Cargo.lock");
const DESKTOP_SOURCE: &str = include_str!("../runtime/desktop/src/main.rs");
const DESKTOP_ASSETS: &str = include_str!("../runtime/desktop/src/assets.rs");
const WELCOME: &str = include_str!("../runtime/desktop/welcome.html");
const ANDROID_SETTINGS: &str = include_str!("../runtime/android/settings.gradle.kts");
const ANDROID_BUILD: &str = include_str!("../runtime/android/build.gradle.kts");
const ANDROID_PROPERTIES: &str = include_str!("../runtime/android/gradle.properties");
const ANDROID_MANIFEST: &str = include_str!("../runtime/android/src/main/AndroidManifest.xml");
const ANDROID_ACTIVITY: &str = include_str!("../runtime/android/src/main/kotlin/MainActivity.kt");
const SERVO_AAR: &str =
    "https://github.com/servo/servo/releases/download/v0.8.0/servo-aarch64-android.aar";
const PACKAGE_MARKER: &str = ".proton-package";

#[derive(Clone, Copy)]
pub enum Target {
    Windows,
    Linux,
    Android,
}

impl Target {
    fn directory(self) -> &'static str {
        match self {
            Self::Windows => "win",
            Self::Linux => "linux",
            Self::Android => "android",
        }
    }

    fn desktop_spec(self) -> io::Result<(&'static str, &'static str, &'static str)> {
        match (self, env::consts::OS, env::consts::ARCH) {
            (Self::Windows, "windows", "x86_64") => {
                Ok(("x86_64-pc-windows-msvc", "proton.exe", "Proton.exe"))
            }
            (Self::Linux, "linux", "x86_64") => {
                Ok(("x86_64-unknown-linux-gnu", "proton", "proton"))
            }
            (Self::Linux, "linux", "aarch64") => {
                Ok(("aarch64-unknown-linux-gnu", "proton", "proton"))
            }
            (Self::Windows, _, _) => Err(io::Error::other(
                "Build for Windows x64 on a Windows x64 host.",
            )),
            (Self::Linux, _, _) => Err(io::Error::other(
                "Build for Linux on a Linux x64 or ARM64 host. On Windows, run pb inside WSL.",
            )),
            (Self::Android, _, _) => unreachable!("Android has a separate build"),
        }
    }
}

pub fn run(target: Target, project: &Path, welcome: bool) -> io::Result<()> {
    let web = project.join("web");
    if !welcome && !web.join("index.html").is_file() {
        return Err(io::Error::other(
            "web/index.html was not found. Run pb init first.",
        ));
    }

    let platform = target.directory();
    let cache = project.join(".proton").join(platform);
    let dist = project.join("dist");
    let output = dist.join(platform);
    fs::create_dir_all(&cache)?;
    fs::create_dir_all(&dist)?;
    let _lock = lock_build(&cache)?;
    ensure_owned_output(&output)?;
    let stage = Scratch::create(&dist, platform)?;

    let entry = match target {
        Target::Android => android(project, &cache, &stage.0, &web, welcome)?,
        _ => desktop(target, &cache, &stage.0, &web, welcome)?,
    };
    fs::write(stage.0.join(PACKAGE_MARKER), "Proton Batcher\n")?;
    publish(&stage.0, &output)?;
    println!("{}", output.join(entry).display());
    Ok(())
}

fn desktop(
    target: Target,
    cache: &Path,
    output: &Path,
    web: &Path,
    welcome: bool,
) -> io::Result<&'static str> {
    let (triple, binary, packaged_name) = target.desktop_spec()?;
    if !welcome {
        copy_assets(web, &output.join("web"))?;
    }
    write_templates(
        cache,
        &[
            ("Cargo.toml", DESKTOP_MANIFEST),
            ("Cargo.lock", DESKTOP_LOCK),
            ("src/main.rs", DESKTOP_SOURCE),
            ("src/assets.rs", DESKTOP_ASSETS),
            ("welcome.html", WELCOME),
        ],
    )?;
    let target_dir = match env::var_os("CARGO_TARGET_DIR") {
        Some(path) => env::current_dir()?.join(path),
        None => cache.join("target"),
    };
    let mut command = Command::new(env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo")));
    command
        .current_dir(cache)
        .args([
            "build",
            "--release",
            "--locked",
            "--target",
            triple,
            "--target-dir",
        ])
        .arg(&target_dir);
    run_command(&mut command, "Cargo")?;
    fs::copy(
        target_dir.join(triple).join("release").join(binary),
        output.join(packaged_name),
    )?;
    Ok(packaged_name)
}

fn android(
    project: &Path,
    cache: &Path,
    output: &Path,
    web: &Path,
    welcome: bool,
) -> io::Result<&'static str> {
    if env::var_os("ANDROID_HOME").is_none() && env::var_os("ANDROID_SDK_ROOT").is_none() {
        return Err(io::Error::other(
            "Set ANDROID_HOME or ANDROID_SDK_ROOT to the Android SDK directory.",
        ));
    }
    let gradle = env::var_os("PROTON_GRADLE").unwrap_or_else(|| {
        OsString::from(if cfg!(windows) {
            "gradle.bat"
        } else {
            "gradle"
        })
    });
    let mut version = Command::new(&gradle);
    run_command(
        version.arg("--version").stdout(process::Stdio::null()),
        "Gradle 9.8.2",
    )?;
    let assets = Scratch::create(cache, "assets")?;
    let assets_web = assets.0.join("proton-web");
    if welcome {
        fs::create_dir(&assets_web)?;
        fs::write(assets_web.join("index.html"), WELCOME)?;
    } else {
        copy_assets(web, &assets_web)?;
    }
    write_templates(
        cache,
        &[
            ("settings.gradle.kts", ANDROID_SETTINGS),
            ("build.gradle.kts", ANDROID_BUILD),
            ("gradle.properties", ANDROID_PROPERTIES),
            ("src/main/AndroidManifest.xml", ANDROID_MANIFEST),
            ("src/main/kotlin/MainActivity.kt", ANDROID_ACTIVITY),
        ],
    )?;

    let aar = if let Some(path) = env::var_os("PROTON_SERVO_AAR") {
        let path = project.join(path);
        if !path.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Servo AAR not found at {}", path.display()),
            ));
        }
        path
    } else {
        let aar = cache.join("servo.aar");
        if !aar.is_file() {
            let download = Scratch::create(cache, "download")?;
            let temporary_aar = download.0.join("servo.aar");
            run_command(
                Command::new("curl")
                    .args([
                        "--fail",
                        "--silent",
                        "--show-error",
                        "--location",
                        "--retry",
                        "2",
                        "--output",
                    ])
                    .arg(&temporary_aar)
                    .arg(SERVO_AAR),
                "Servo AAR download",
            )?;
            fs::rename(temporary_aar, &aar)?;
        }
        aar
    };

    let mut command = Command::new(gradle);
    command
        .current_dir(cache)
        .env("PROTON_ASSETS_DIR", &assets.0)
        .env("PROTON_SERVO_AAR", &aar)
        .arg(format!(
            "-PprotonAppId={}",
            android_app_id(project, welcome)
        ))
        .args(["--no-daemon", "assembleDebug"]);
    run_command(&mut command, "Android APK build")?;

    let name = "Proton-debug.apk";
    fs::copy(
        cache.join("build/outputs/apk/debug/proton-android-debug.apk"),
        output.join(name),
    )?;
    Ok(name)
}

fn android_app_id(project: &Path, welcome: bool) -> String {
    if welcome {
        return "app.proton.welcome".into();
    }
    let name = project.file_name().unwrap_or_default().to_string_lossy();
    let slug: String = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    format!("app.proton.project_{slug}")
}

fn run_command(command: &mut Command, label: &str) -> io::Result<()> {
    let status = command.status().map_err(|error| {
        io::Error::new(error.kind(), format!("Could not run {label} ({error})"))
    })?;
    if !status.success() {
        return Err(io::Error::other(format!("{label} failed ({status}).")));
    }
    Ok(())
}

fn write_templates(root: &Path, templates: &[(&str, &str)]) -> io::Result<()> {
    for (name, contents) in templates {
        let path = root.join(name);
        if fs::read(&path).is_ok_and(|existing| existing == contents.as_bytes()) {
            continue;
        }
        fs::create_dir_all(path.parent().expect("template path has a parent"))?;
        fs::write(path, contents)?;
    }
    Ok(())
}

fn copy_assets(source: &Path, destination: &Path) -> io::Result<()> {
    if !fs::symlink_metadata(source)?.is_dir() {
        return Err(io::Error::other(format!(
            "Expected a directory at {}",
            source.display()
        )));
    }
    fs::create_dir(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let destination = destination.join(entry.file_name());
        if kind.is_dir() {
            copy_assets(&entry.path(), &destination)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), destination)?;
        } else {
            return Err(io::Error::other(format!(
                "Unsupported file type at {}",
                entry.path().display()
            )));
        }
    }
    Ok(())
}

fn publish(stage: &Path, output: &Path) -> io::Result<()> {
    ensure_owned_output(output)?;
    let parent = output.parent().expect("output has a dist parent");
    let backup = Scratch::create(parent, "backup")?;
    let previous = backup.0.join("package");
    let had_previous = match fs::rename(output, &previous) {
        Ok(()) => true,
        Err(error) if error.kind() == io::ErrorKind::NotFound => false,
        Err(error) => return Err(error),
    };
    if let Err(error) = fs::rename(stage, output) {
        if had_previous && let Err(restore_error) = fs::rename(&previous, output) {
            let path = previous.display().to_string();
            std::mem::forget(backup);
            return Err(io::Error::other(format!(
                "Could not publish the package ({error}). The previous package is at {path} ({restore_error})."
            )));
        }
        return Err(error);
    }
    Ok(())
}

fn ensure_owned_output(output: &Path) -> io::Result<()> {
    match fs::symlink_metadata(output) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
        Ok(info) if info.is_dir() && output.join(PACKAGE_MARKER).is_file() => Ok(()),
        Ok(_) => Err(io::Error::other(format!(
            "{} already exists. Move the existing directory before building again.",
            output.display()
        ))),
    }
}

struct Scratch(PathBuf);

impl Scratch {
    fn create(parent: &Path, name: &str) -> io::Result<Self> {
        let path = parent.join(format!(".{name}-{}", process::id()));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn lock_build(cache: &Path) -> io::Result<File> {
    let path = cache.join("build.lock");
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    file.try_lock().map_err(|error| match error {
        TryLockError::WouldBlock => io::Error::new(
            io::ErrorKind::WouldBlock,
            "A build is already running for this target.",
        ),
        TryLockError::Error(error) => error,
    })?;
    Ok(file)
}
