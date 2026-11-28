# The library is generated with --autosar, so it needs the ara::core headers
# of your AUTOSAR installation (or the AUTOSAR CAPI core/src/core-types/include).
# For a compile test with minimal stubs use
#   make ARA_INCLUDE=-I../../cpp/test_headers/ara-stub
ifndef ARA_INCLUDE
$(error set ARA_INCLUDE=-I<dir containing ara/core/*.h>)
endif
CXXFLAGS=-I../../cpp/helper-types -I../../cpp/test_headers $(ARA_INCLUDE) -g -fPIC -std=c++17

all: libruntime.a

libruntime.a: module.o executor_import.o
	${AR} rcuvs $@ $^

clean:
	-rm module.o libruntime.a
