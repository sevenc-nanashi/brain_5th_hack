#include "opal.h"
static Opal chip;
void brain_opl_reset(void) {
    chip = (Opal){0};
    Opal_Init(&chip, 11025);
}
void brain_opl_write(unsigned short reg, unsigned char value) {
    Opal_Port(&chip, reg, value);
}
void brain_opl_sample(short *out) { Opal_Sample(&chip, out, out + 1); }
