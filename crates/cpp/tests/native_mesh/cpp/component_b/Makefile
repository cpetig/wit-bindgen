CXXFLAGS=-fPIC -g -O0 -I../../../../helper-types -std=c++1z

all: libcomponent_b.so

libcomponent_b.so: b.cpp impl.cpp
	$(CXX) -shared $(CXXFLAGS) -o $@ $^

bindgen:
	../../../../../target/debug/wit-bindgen cpp ../wit -w b --wasm64 --format

clean:
	-rm *~ *.so *.o
