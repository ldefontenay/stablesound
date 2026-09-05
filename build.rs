//! Compiles the dialog template into the executable.
//!
//! `embed-resource` is a *build* dependency: it runs rc.exe at build time and
//! nothing of it ships. What ships is the compiled template, a couple of
//! kilobytes, which is the cheapest way to get real Win32 controls - see the
//! note at the top of `stablesound.rc`.
fn main() {
    embed_resource::compile("stablesound.rc", embed_resource::NONE)
        .manifest_required()
        .unwrap();
    println!("cargo:rerun-if-changed=stablesound.rc");
}
