CFLAGS += -std=gnu11 -D_POSIX_C_SOURCE=200809L -D_DEFAULT_SOURCE -D_BSD_SOURCE -Os -ffunction-sections -fdata-sections -fno-strict-aliasing
CFLAGS += -I$(PREFIX)/include -Ifrontends/brain
CFLAGS += $(BRAIN_CFLAGS)
LDFLAGS += -L$(PREFIX)/lib
S_FRONTEND := bridge.c plot.c font.c fetch.c
ifeq ($(HOST),arm-none-eabi)
S_FRONTEND += runtime.c
endif
SOURCES = $(filter-out content/fetchers/file/file.c,$(S_COMMON)) $(S_IMAGE) $(S_BROWSER) $(S_FRONTEND)
EXETARGET := unused
MESSAGES_FILTER := fb
MESSAGES_TARGET := frontends/brain/res

.SECONDEXPANSION:
brain-archive: $$(OBJECTS)
	$(AR) crs $(PREFIX)/lib/libbrain-netsurf.a $(OBJECTS)
