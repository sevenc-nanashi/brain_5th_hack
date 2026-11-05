# Loaded by build-tic80.rb; patch only the private build copy.
def build_luajit(inputs, target, out, port)
  jit = File.join(out, 'luajit')
  FileUtils.rm_rf(jit)
  FileUtils.cp_r(File.join(inputs, 'luajit'), jit)
  src = File.join(jit, 'src')
  # A volatile counter in each trace also bounds loops that never call Lua hooks.
  patched(File.join(src, 'lj_record.c'), File.join(src, 'lj_record.c'), {
    'IRRef ref = REF_FIRST + LJ_HASPROFILE;' => 'IRRef ref = REF_FIRST + LJ_HASPROFILE + 4;',
    "#if LJ_HASPROFILE\n  J->prev_pt = NULL;" => <<~C.rstrip
      {
        extern volatile int brain_tic_trace_budget;
        TRef ptr = lj_ir_kptr(J, (void *)&brain_tic_trace_budget);
        TRef count = emitir(IRT(IR_XLOAD, IRT_INT), ptr, IRXLOAD_VOLATILE);
        emitir(IRTGI(IR_GT), count, lj_ir_kint(J, 0));
        count = emitir(IRTI(IR_SUB), count, lj_ir_kint(J, 1));
        emitir(IRT(IR_XSTORE, IRT_INT), ptr, count);
      }
      #if LJ_HASPROFILE
        J->prev_pt = NULL;
    C
  })
  options = %w[-DLUAJIT_DISABLE_FFI -DLUAJIT_DISABLE_PROFILE -DLUAJIT_ENABLE_LUA52COMPAT]
  args = ['make', '-C', src, '-j2', 'libluajit.a', 'BUILDMODE=static', 'CC=clang-20', 'TARGET_AR=llvm-ar-20 rcus']
  if target == 'arm'
    patched(File.join(src, 'lj_mcode.c'), File.join(src, 'lj_mcode.c'), {
      '#error "Missing OS support for explicit placement of executable memory"' => File.read(File.join(port, 'engine/luajit-memory.h'))
    })
    options += %w[-DLUAJIT_USE_SYSMALLOC -DLUAJIT_NO_UNWIND -DLUAJIT_SECURITY_PRNG=0 -DLUAJIT_SECURITY_MCODE=0]
    args += ["HOST_CC=#{ENV.fetch('LUAJIT_HOST_CC', 'clang-20 -m32')}", 'TARGET_SYS=Other',
             "STATIC_CC=clang-20 --target=armv7a-none-eabi -mcpu=cortex-a7 -mfloat-abi=softfp -mfpu=vfpv3 -marm -isystem #{File.join(inputs, 'newlib/usr/include/newlib').shellescape}"]
  end
  abort 'Build LuaJIT failed' unless system(*args, "XCFLAGS=#{options.join(' ')}")
  FileUtils.cp(File.join(src, 'libluajit.a'), File.join(out, 'libbrain-luajit.a'))
  jit
end
