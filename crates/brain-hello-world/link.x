OUTPUT_FORMAT("elf32-littlearm")
OUTPUT_ARCH(arm)
ENTRY(_start)

MEMORY {
    APP (rx) : ORIGIN = 0x90000000, LENGTH = 15M
}

SECTIONS {
    .text : {
        KEEP(*(.text.entry))
        *(.text .text.*)
        *(.rodata .rodata.*)
        . = ALIGN(4);
    } > APP
    .data : { *(.data .data.*) } > APP
    .bss (NOLOAD) : { *(.bss .bss.*) *(COMMON) } > APP
    /DISCARD/ : { *(.ARM.exidx*) *(.ARM.extab*) *(.comment) }
}

ASSERT(_start == 0x90000000, "AppMain.bin must start at the ARM entry point")
ASSERT(SIZEOF(.data) == 0 && SIZEOF(.bss) == 0, "Startup does not initialize writable statics")
