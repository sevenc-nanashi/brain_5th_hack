OUTPUT_FORMAT("elf32-littlearm")
OUTPUT_ARCH(arm)
ENTRY(_start)
MEMORY { APP (rwx) : ORIGIN = 0x90000000, LENGTH = 15M }
SECTIONS {
    .text : {
        KEEP(*(.text.entry))
        *(.text .text.*)
        *(.rodata .rodata.*)
        . = ALIGN(16);
    } > APP
    .data 0x90500000 : AT(LOADADDR(.text) + SIZEOF(.text)) {
        __data_start = .;
        *(.data .data.*)
        . = ALIGN(16);
        __data_end = .;
    } > APP
    __data_load = LOADADDR(.data);
    .bss (NOLOAD) : {
        __bss_start = .;
        *(.bss .bss.*) *(COMMON)
        . = ALIGN(16);
        __bss_end = .;
    } > APP
    /DISCARD/ : { *(.ARM.exidx*) *(.ARM.extab*) *(.eh_frame*) *(.comment) }
}
ASSERT(_start == 0x90000000, "AppMain.bin must start at the ARM entry point")
ASSERT(LOADADDR(.data) + SIZEOF(.data) <= 0x90500000, "Loaded image overlaps app data")
ASSERT(__bss_end <= 0x90f00000, "App exceeds the additional-app region")
