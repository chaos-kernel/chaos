use std::{env, fs, path::PathBuf};
fn main() {
    let ld = &PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("linker.ld");
    fs::write(ld, LINKER).unwrap();
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-link-arg=-T{}", ld.display());
}

const LINKER: &[u8] = b"
OUTPUT_ARCH(riscv)
ENTRY(_start)
SECTIONS {
    . = 0x40200000;
    .text : {
        *(.text.entry)
        *(.text .text.*)
    }
    .rodata : {
        *(.rodata .rodata.*)
        *(.srodata .srodata.*)
    }
    .data : {
        *(.data .data.*)
        *(.sdata .sdata.*)
    }
    .bss (NOLOAD) : {
        *(.bss.uninit)
        sbss = .;
        *(.bss .bss.*)
        *(.sbss .sbss.*)
        . = ALIGN(8);
        ebss = .;
    }
    /DISCARD/ : {
        *(.eh_frame)
    }
}";