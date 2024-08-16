static TARGET_PATH: &str = "../user/target/riscv64gc-unknown-none-elf/release/";

fn main() {
    println!("cargo:rerun-if-changed=../user/src/");
    println!("cargo:rerun-if-changed={}", TARGET_PATH);

    #[cfg(feature = "qemu")]
    println!("cargo:-Clink-arg=-Tsrc/linker.ld");

    #[cfg(feature = "visionfive2")]
    println!("cargo:-Clink-arg=-Tsrc/linker-vf2.ld");

    println!("cargo:rerun-if-changed=src/linker.ld");
    println!("cargo:rerun-if-changed=src/linker-vf2.ld");
}
