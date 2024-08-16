    .section .text.entry
    .globl _start
_start:
    # pc = qemu: 0x80200000
    #      visionfive2: 0x40200000

    la sp, boot_stack_top

    # since the base addr is 0xffff_ffc0_4020_0000
    # we need to activate pagetable here in case of absolute addressing
    # satp: 8 << 60 | boot_pagetable
    la t0, boot_pagetable
    li t1, 8 << 60
    srli t0, t0, 12
    or t0, t0, t1
    csrw satp, t0
    sfence.vma

    call fake_main

    .section .bss.stack
    .globl boot_stack_lower_bound
boot_stack_lower_bound:
    .space 4096 * 16
    .globl boot_stack_top
boot_stack_top:

    .section .data
    .align 12
boot_pagetable:
    # we need 2 pte here
    # 0x0000_0000_4000_0000 -> 0x0000_0000_4000_0000
    # 0xffff_fc00_4000_0000 -> 0x0000_0000_4000_0000
    .quad (0x00000 << 10) | 0xcf # VRWXAD 1G大页
    .quad (0x40000 << 10) | 0xcf # VRWXAD 1G大页
    .quad (0x80000 << 10) | 0xcf # VRWXAD 1G大页
    .zero 8 * 253
    .quad (0x00000 << 10) | 0xcf # VRWXAD 1G大页
    .quad (0x40000 << 10) | 0xcf # VRWXAD 1G大页
    .quad (0x80000 << 10) | 0xcf # VRWXAD 1G大页
    .zero 8 * 253

message:
    .string "Hello, World!"