OUTPUT_FORMAT("elf32-littlearm")
OUTPUT_ARCH(arm)
ENTRY(_start)
MEMORY {
    CODE (rx) : ORIGIN = 0x83400000, LENGTH = 4M
    DATA (rw) : ORIGIN = 0x86c00000, LENGTH = 4M
}
SECTIONS {
    .text : {
        KEEP(*(.text.entry))
        *(.text .text.*) *(.rodata .rodata.*)
        . = ALIGN(16);
    } > CODE
    .data : AT(LOADADDR(.text) + SIZEOF(.text)) {
        __data_start = .;
        *(.data .data.*)
        . = ALIGN(16);
        __data_end = .;
    } > DATA
    __data_load = LOADADDR(.data);
    .bss (NOLOAD) : {
        __bss_start = .;
        *(.bss .bss.*) *(COMMON)
        . = ALIGN(16);
        __bss_end = .;
    } > DATA
    /DISCARD/ : { *(.ARM.exidx*) *(.ARM.extab*) *(.eh_frame*) *(.comment) }
}
ASSERT(_start == 0x83400000, "Resident requires CDTimer slot 3")
ASSERT(LOADADDR(.data) + SIZEOF(.data) <= 0x83800000, "Resident exceeds code slot")
ASSERT(__bss_end <= 0x87000000, "Resident overlaps eNote data")
