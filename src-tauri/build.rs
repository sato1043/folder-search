fn main() {
    tauri_build::build();

    // Windows MSVC: llama-cpp-sys-2のwrapper(cc crate, /MD)と
    // esaxx-rs(/MT hardcoded)のCRT不一致を解消。
    // msvcprt.lib(動的C++ランタイム)を除外し、libcpmt.lib(静的)に統一。
    // ort/llamaのcmakeオブジェクトはDLL化済みで影響なし。
    #[cfg(all(target_os = "windows", target_env = "msvc"))]
    {
        println!("cargo:rustc-link-arg=/NODEFAULTLIB:msvcprt.lib");
        println!("cargo:rustc-link-arg=/FORCE");
    }
}
