
extern bool brain_tic_budget(void);
void brain_tic_hook(lua_State *lua, lua_Debug *debug) {
    if (!brain_tic_budget()) luaL_error(lua, "Frame instruction limit exceeded");
}
