fn main() {
    tauri_build::build();

    // Windows MSVC: ort-sysプリビルド静的ライブラリ(MD/動的CRT)と
    // cc crateデフォルト(MT/静的CRT)のCRT競合を解消
    #[cfg(all(target_os = "windows", target_env = "msvc"))]
    {
        println!("cargo:rustc-link-arg=/NODEFAULTLIB:msvcprt.lib");
        println!("cargo:rustc-link-arg=/FORCE:MULTIPLE");
    }
}
