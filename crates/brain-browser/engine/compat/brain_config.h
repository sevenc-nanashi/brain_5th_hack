#include "utils/config.h"
#ifdef BRAIN_ARM
#undef HAVE_MMAP
#undef WITH_MMAP
#undef HAVE_UTSNAME
#undef HAVE_DIRFD
#undef HAVE_UNLINKAT
#undef HAVE_FSTATAT
#undef HAVE_SCANDIR
#undef HAVE_STRPTIME
#undef HAVE_STRFTIME
#endif
