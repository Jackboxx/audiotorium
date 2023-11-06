fn main() {
    let url = if cfg!(not(debug_assertions)) {
        "sqlite:prod.db"
    } else {
        "sqlite:dev.db"
    };

    println!("cargo:rustc-env=DATABASE_URL={url}");
}
