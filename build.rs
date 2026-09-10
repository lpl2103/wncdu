//! Build script to embed Windows application icon and version metadata into the binary.

fn main() {
    println!("cargo:rerun-if-changed=assets/winncdu.rc");
    println!("cargo:rerun-if-changed=assets/winncdu.ico");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" {
        let out_dir = std::env::var("OUT_DIR").unwrap_or_default();
        let res_file = std::path::Path::new(&out_dir).join("winncdu.res");

        let windres_candidates = [
            "C:/Users/Leandro/AppData/Local/Microsoft/WinGet/Packages/BrechtSanders.WinLibs.POSIX.MSVCRT_Microsoft.Winget.Source_8wekyb3d8bbwe/mingw64/bin/windres.exe",
            "x86_64-w64-mingw32-windres",
            "windres",
        ];

        let mut compiled = false;
        for cmd in &windres_candidates {
            let success = std::process::Command::new(cmd)
                .args([
                    "-i",
                    "assets/winncdu.rc",
                    "-O",
                    "coff",
                    "-o",
                    res_file.to_str().unwrap_or("winncdu.res"),
                ])
                .status()
                .is_ok_and(|s| s.success());

            if success {
                println!("cargo:rustc-link-arg={}", res_file.display());
                compiled = true;
                break;
            }
        }

        // Fallback to pre-compiled resource if windres was not invocable
        if !compiled && std::path::Path::new("assets/winncdu.res").exists() {
            let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
            let precompiled = std::path::Path::new(&manifest_dir).join("assets/winncdu.res");
            println!("cargo:rustc-link-arg={}", precompiled.display());
        }
    }
}
