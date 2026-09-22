//! ScreenCaptureKit's Swift bridge links against the system Swift runtime. Cargo does not pass a
//! dependency's link arguments on, so this crate's tests and examples need the rpath themselves.
//! (Any binary linking this crate, the app included, needs the same `-rpath /usr/lib/swift`.)
fn main() {
    println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
}
