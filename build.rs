fn main() {
    #[cfg(all(target_os = "linux", target_env = "musl"))]
    {
        println!("cargo:rustc-link-lib=gcc_eh");
    }
}
