// Packs a mod folder into a watane package file: demo-pack <mod dir> <out file>.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let p = watane_pkg::pack(std::path::Path::new(&a[1]), false).unwrap();
    std::fs::write(&a[2], p.as_bytes()).unwrap();
    println!("{}", p.id());
}
