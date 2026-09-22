fn main() {
    // screencapturekit's Swift bridge links the Swift runtime, which macOS ships in /usr/lib/swift.
    println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
    tauri_build::build()
}
