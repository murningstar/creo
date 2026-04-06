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

    tauri_build::build()
}
