fn main() {
    tauri_build::build();

    // Windows MSVC: ort-sysプリビルド静的ライブラリ(MD/動的CRT)と
    // cc crateデフォルト(MT/静的CRT)のCRT多重定義をwarningに降格
    #[cfg(all(target_os = "windows", target_env = "msvc"))]
    println!("cargo:rustc-link-arg=/FORCE");
}
