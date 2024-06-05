CXXFLAGS=-g -O0 -I../../helper-types -I../../cpp_host -I../../guest --std=c++17
WIT_BINDGEN=../../../../target/debug/wit-bindgen

all: libstrings.so cpp-app-strings rust-app-strings

libstrings.so:
	cd guest && make $@
	
cpp-app-strings: ./cpp_host/the_world_native.o main.o ./cpp_host/guest_imported_fns.o
	$(CXX) $(CXXFLAGS) -o $@ $^ -g -L./guest -lstrings

rust-app-strings:  main.o
	$(CXX) $(CXXFLAGS) -o $@ $^ -g -L/mnt/sda2/pradeep/wasm/RND/math_fns/target/debug -lstrings


the_world_native.o:
	cd cpp_host && make $@

bindgen: wit/strings.wit
	$(WIT_BINDGEN) cpp wit --wasm64 --format
	$(WIT_BINDGEN) cpp wit --wasm64 --format --direct
	cd rust/src ; ../../$(WIT_BINDGEN) rust ../../wit --wasm64

guest.wasm: the_world.cpp guest_1.cpp
	/opt/wasi-sdk/bin/clang++ -o $@ $^ $(CXXFLAGS) 
	
guest_release.wasm: the_world.cpp guest_1.cpp
	/opt/wasi-sdk/bin/clang++ -o $@ $^ $(CXXFLAGS) -g0 -O3
	
clean:
	-rm *.o app-strings
	cd guest && make clean
	cd cpp_host && make clean

run:
	LD_LIBRARY_PATH=. ./app-strings

valgrind:
	LD_LIBRARY_PATH=. valgrind ./app-strings

w2c2_guest.c: guest_release.wasm
	w2c2 $^ $@
