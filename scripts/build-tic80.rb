#!/usr/bin/env ruby
require 'fileutils'

root = File.expand_path('..', __dir__)
inputs = File.expand_path(ARGV.fetch(0))
target = ARGV.fetch(1)
abort 'Expected host or arm' unless %w[host arm].include?(target)
out = File.expand_path(ARGV.fetch(2))
port = File.join(root, 'crates/brain-tic80')
tic = File.join(inputs, 'tic80')
lua = File.join(inputs, 'lua')
blip = File.join(inputs, 'blip')
zlib = File.join(inputs, 'zlib')
abort 'Run scripts/fetch-tic80.rb first' unless File.file?(File.join(tic, 'src/core/core.c'))
FileUtils.mkdir_p(out)

# Patch generated copies; downloaded sources remain reproducible.
def patched(source, output, replacements, expected: 1)
  code = File.read(source)
  replacements.each do |before, after|
    raise "Patch no longer applies: #{source}: #{before}" unless code.scan(before).size == expected
    code.gsub!(before, after)
  end
  File.write(output, code)
  output
end
lua_api = patched(File.join(tic, 'src/api/lua.c'), File.join(out, 'lua-api.c'), {
  'return (s32)lua_tonumber(lua, index);' => <<~C.strip,
    if (lua_isinteger(lua, index)) {
        lua_Integer value = lua_tointeger(lua, index);
        if (value >= INT32_MIN && value <= INT32_MAX) return (s32)value;
    }
    return (s32)lua_tonumber(lua, index);
  C
  'lua_State* lua = core->currentVM = luaL_newstate();' => <<~C.strip
    lua_State* lua = core->currentVM = luaL_newstate();
    if (!lua) { core->data->error(core->data->data, "Lua allocation failed"); return false; }
    extern void brain_tic_hook(lua_State*, lua_Debug*);
    lua_sethook(lua, brain_tic_hook, LUA_MASKCOUNT, 10000);
  C
})
core = patched(File.join(tic, 'src/core/core.c'), File.join(out, 'core.c'), {
  "// if it couldn't init, make sure the VM is not left dirty by the implementation\n        core->currentVM = NULL;" => "config->close((tic_mem*)core);\n        core->currentVM = NULL;",
  'free(memory->product.samples.buffer);' => "free(memory->product.samples.buffer);\n    free(memory->base_ram);"
})
cart = patched(File.join(tic, 'src/cart.c'), File.join(out, 'cart.c'), {
  'const Chunk* chunk = (Chunk*)ptr;' => 'Chunk header; memcpy(&header, ptr, sizeof header); const Chunk* chunk = &header;'
}, expected: 2)
# Count-only hooks need the slow path only when the instruction budget expires.
lua_vm = patched(File.join(lua, 'lvm.c'), File.join(out, 'lvm.c'), {
  'Protect(luaG_traceexec(L));' => 'if ((L->hookmask & LUA_MASKLINE) || L->hookcount <= 1) { Protect(luaG_traceexec(L)); } else { --L->hookcount; }'
})
flags = %w[-std=gnu11 -O2 -fno-strict-aliasing -fwrapv -ffunction-sections -fdata-sections -DTIC_BUILD_WITH_LUA=1 -DLUA_COMPAT_5_2]
[File.join(tic, 'include'), File.join(tic, 'src'), File.join(tic, 'src/core'), lua, blip, zlib].each { |d| flags += ['-I', d] }
if target == 'arm'
  flags += %w[--target=armv7a-none-eabi -mcpu=cortex-a7 -mfloat-abi=soft -marm -ffreestanding -fno-builtin -fno-stack-protector -flto=thin -DBRAIN_ARM]
  flags += ['-isystem', File.join(inputs, 'newlib/usr/include/newlib')]
end
sources = %w[core/languages.c core/draw.c core/io.c core/sound.c tools.c zip.c tilesheet.c].map { |f| File.join(tic, 'src', f) }
sources += [core, cart, lua_api, File.join(blip, 'blip_buf.c'), File.join(port, 'engine/bridge.c')]
sources += Dir.glob(File.join(lua, '*.c')).reject { |p| %w[lua.c onelua.c ltests.c].include?(File.basename(p)) }
sources[sources.index(File.join(lua, 'lvm.c'))] = lua_vm
sources += %w[adler32.c crc32.c inffast.c inflate.c inftrees.c uncompr.c zutil.c].map { |f| File.join(zlib, f) }
sources << File.join(port, 'engine/runtime.c') if target == 'arm'
objects = sources.each_with_index.map do |source, i|
  object = File.join(out, "tic-#{i}.o")
  abort "Compile failed: #{source}" unless system('clang-20', *flags, '-c', source, '-o', object)
  object
end
archive = File.join(out, 'libbrain-tic80.a')
FileUtils.rm_f(archive)
abort 'Archive failed' unless system('llvm-ar-20', 'crs', archive, *objects)

code = File.binread(File.join(port, 'assets/demo.lua'))
raise 'Demo exceeds one code bank' unless code.bytesize.between?(1, 65535)
File.binwrite(File.join(inputs, 'cart.tic'), [17, 0, 0, 0, 5, code.bytesize & 255, code.bytesize >> 8, 0].pack('C*') + code)
