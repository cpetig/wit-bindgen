CXXFLAGS=-g -O0 -I../../../helper-types --std=c++17
WIT_BINDGEN=../../../../target/debug/wit-bindgen

all: the_world_native.o guest_imported_fns.o

the_world_native.o:
	$(CXX) $(CXXFLAGS) -fPIE -o $@ -c the_world_native.cpp

guest_imported_fns.o:
	$(CXX) $(CXXFLAGS) -fPIE -o $@ -c guest_imported_fns.cpp


clean:
	-rm *.o