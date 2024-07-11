
    .section .data
    .global initproc_start
    .global initproc_end
initproc_start:
    .incbin "../user/target/riscv64gc-unknown-none-elf/release/initproc"
initproc_end:
