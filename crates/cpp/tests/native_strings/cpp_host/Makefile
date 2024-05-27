CXXFLAGS=-g -O0 -I../../../helper-types --std=c++17
WIT_BINDGEN=../../../../target/debug/wit-bindgen

all: the_world_native.o

the_world_native.o:
	$(CXX) $(CXXFLAGS) -fPIE -o $@ -c the_world_native.cpp

clean:
	-rm *.o