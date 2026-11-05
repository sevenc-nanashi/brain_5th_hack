#include <lua.h>
#include <lauxlib.h>
#include <lualib.h>
#include <luajit.h>

// LuaJIT opens coroutine with base and provides the remaining 5.2 APIs used here.
static int luaopen_coroutine(lua_State *lua) {
    lua_getglobal(lua, LUA_COLIBNAME);
    return 1;
}
static int brain_luajit_rawgeti(lua_State *lua, int index, int key) {
    lua_rawgeti(lua, index, key);
    return lua_type(lua, -1);
}
#define lua_rawgeti brain_luajit_rawgeti
static int lua_isinteger(lua_State *lua, int index) {
    return lua_type(lua, index) == LUA_TNUMBER && lua_tonumber(lua, index) == (lua_Number)lua_tointeger(lua, index);
}
static void luaL_requiref(lua_State *lua, const char *name, lua_CFunction open, int global) {
    lua_pushcfunction(lua, open);
    lua_pushstring(lua, name);
    lua_call(lua, 1, 0);
    lua_getglobal(lua, name);
}
