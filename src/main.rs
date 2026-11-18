mod build;

use std::env;
use std::ffi::OsStr;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

const STARTER: &str = include_str!("../templates/starter.html");
const HELP: &str = "Proton Batcher (pb) — Proton.js build tool

  pb init [directory]           Create a project with web/index.html
  pb -build -win [--welcome]    Windows x64 EXE
  pb -build -linux [--welcome]  Linux x64 or ARM64 app directory
  pb -build -android [--welcome] Android ARM64 debug APK
  pb --version                  Show the version

Run build commands from your project directory. Use --welcome to package
the Proton.js welcome screen. Build output goes into dist/.";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("pb: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> io::Result<()> {
    let arguments: Vec<_> = env::args_os().skip(1).collect();
    let args: Vec<_> = arguments
        .iter()
        .map(|argument| argument.as_os_str())
        .collect();
    match args.as_slice() {
        [] => println!("{HELP}"),
        [option] if ["--help", "-h", "help"].iter().any(|alias| option == alias) => {
            println!("{HELP}");
        }
        [option] if *option == "--version" || *option == "-V" => {
            println!("pb {}", env!("CARGO_PKG_VERSION"));
        }
        [command] if *command == "init" => init(Path::new("."))?,
        [command, directory] if *command == "init" => init(Path::new(directory))?,
        [command, platform] if *command == "-build" || *command == "build" => {
            build::run(target(platform)?, &env::current_dir()?, false)?;
        }
        [command, platform, option]
            if (*command == "-build" || *command == "build") && *option == "--welcome" =>
        {
            build::run(target(platform)?, &env::current_dir()?, true)?;
        }
        _ => {
            return Err(io::Error::other(
                "Invalid command. Run pb --help for usage.",
            ));
        }
    }
    Ok(())
}

fn target(platform: &OsStr) -> io::Result<build::Target> {
    match platform.to_str() {
        Some("-win" | "win") => Ok(build::Target::Windows),
        Some("-linux" | "linux") => Ok(build::Target::Linux),
        Some("-android" | "android") => Ok(build::Target::Android),
        _ => Err(io::Error::other(
            "Build target must be -win, -linux, or -android.",
        )),
    }
}

fn init(directory: &Path) -> io::Result<()> {
    let web = directory.join("web");
    fs::create_dir_all(&web)?;
    let entry = web.join("index.html");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&entry)
        .map_err(|err| {
            if err.kind() == io::ErrorKind::AlreadyExists {
                io::Error::new(err.kind(), format!("{} already exists.", entry.display()))
            } else {
                err
            }
        })?;
    file.write_all(STARTER.as_bytes())?;
    println!("{}", entry.display());
    Ok(())
}
