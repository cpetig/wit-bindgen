
all:
	make -C component_b $@
	make -C mesh $@
	make -C component_a $@

bindgen:
	make -C component_b $@
	make -C mesh $@
	make -C component_a $@

clean:
	make -C component_b $@
	make -C mesh $@
	make -C component_a $@
	
run:
	LD_LIBRARY_PATH=mesh component_a/component_a
