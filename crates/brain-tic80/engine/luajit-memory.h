// Requires an executable Brain heap; no per-allocation protection API is available.
#define MCPROT_RW 0
#define MCPROT_RX 0
#define MCPROT_RWX 0
static void *mcode_alloc_at(uintptr_t hint, size_t size, int prot) {
    return malloc(size);
}
static void mcode_free(void *memory, size_t size) {
    free(memory);
}
static void mcode_setprot(jit_State *J, void *memory, size_t size, int prot) {}
