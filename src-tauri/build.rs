fn main() {
    // Warn if compiling with GCC 15 on Linux — whisper.cpp crashes in std::regex
    // (GCC Bug 86164: recursive DFS in libstdc++ std::regex causes stack overflow).
    // Fixed in GCC 16 (Fedora 44). Workaround: CC=clang CXX=clang++.
    #[cfg(target_os = "linux")]
    {
        if let Ok(output) = std::process::Command::new("gcc")
            .arg("--version")
            .output()
        {
            let version = String::from_utf8_lossy(&output.stdout);
            // Match "15." or standalone "15" in version tokens
            // e.g. "gcc (GCC) 15.0.0", "gcc (Ubuntu 15-20250101) 15.0.0"
            if version.split_whitespace().any(|w| w.starts_with("15.") || w == "15") {
                println!(
                    "cargo:warning=GCC 15 detected. whisper.cpp may crash during build \
                     (std::regex stack overflow, GCC Bug 86164). \
                     Workaround: CC=clang CXX=clang++ cargo tauri dev"
                );
            }
        }
    }

    // Vosk: link against prebuilt libvosk from src-tauri/lib/vosk/
    #[cfg(feature = "vosk")]
    {
        let vosk_lib_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("lib/vosk");
        if vosk_lib_dir.exists() {
            println!(
                "cargo:rustc-link-search=native={}",
                vosk_lib_dir.display()
            );
            // Set rpath so the binary finds libvosk.so at runtime during development
            #[cfg(target_os = "linux")]
            println!(
                "cargo:rustc-link-arg=-Wl,-rpath,{}",
                vosk_lib_dir.display()
            );
            #[cfg(target_os = "macos")]
            println!(
                "cargo:rustc-link-arg=-Wl,-rpath,{}",
                vosk_lib_dir.display()
            );
        } else {
            println!(
                "cargo:warning=Vosk feature enabled but lib/vosk/ not found. \
                 Download libvosk from https://github.com/alphacep/vosk-api/releases"
            );
        }
    }

    tauri_build::build()
}
