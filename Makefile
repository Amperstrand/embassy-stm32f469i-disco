# Bench/HIL entry points for the BSP (the Amperstrand bench contract:
# BenchLock FIRST, then the labgrid place, image backup/restore around
# every flash session on the shared F469).
.PHONY: hil-place test-hil test-hil-quick

# (Re)create the bsp-f469-hil labgrid place after coordinator restarts.
hil-place:
	bash tools/hil/labgrid-place.sh

# Full bench-wrapped HIL battery (all run_hil.sh phases).
test-hil:
	python3 tools/hil/bench.py

# Phases 1-2 only (no USB serial dependency).
test-hil-quick:
	python3 tools/hil/bench.py -- --skip usb
