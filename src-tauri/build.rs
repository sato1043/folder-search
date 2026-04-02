fn main() {
    tauri_build::build();

    // Windows MSVC: llama-cpp-sys-2のwrapperコード(cc crate, /MD)と
    // esaxx-rs(cc crate, /MT hardcoded)のCRTメタデータ不一致を抑制。
    // llama.cppのcmakeオブジェクトはdynamic-linkでDLL化済みのため、
    // 残るmismatchはwrapperの数件のみ。
    #[cfg(all(target_os = "windows", target_env = "msvc"))]
    println!("cargo:rustc-link-arg=/FORCE");
}
