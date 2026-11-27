# Async support of C components using the symmetric ABI: the generated bindings
# of the symmetric runtime (see generate.sh) and the emulation of the canonical
# ABI builtins on top of it.
CC ?= cc
AR ?= ar
CFLAGS ?= -g -fPIC -Wall -Wextra -Wno-unused-parameter

OBJS = executor_import.o module.o wit_async_support.o

all: libruntime.a

libruntime.a: $(OBJS)
	$(AR) rcuvs $@ $^

%.o: %.c
	$(CC) $(CFLAGS) -c -o $@ $<

clean:
	-rm -f $(OBJS) libruntime.a

.PHONY: all clean
