#[cfg(feature = "grammars")]
fn main() {
    print!("{}", two_face::acknowledgement::listing().to_md());
}
#[cfg(not(feature = "grammars"))]
fn main() {
    eprintln!("Enable --features grammars to export bundled grammar notices");
}
