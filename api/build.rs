fn main() {
    let url = if cfg!(debug_assertions) {
        "postgres://audio:audiopass@172.18.0.1:50001"
    } else {
        todo!("read url from env")
    };

    println!("cargo:rustc-env=DATABASE_URL={url}");
}
