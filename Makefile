# BTC Miner Makefile

CXX ?= g++
CXXFLAGS := -std=c++17 -O3 -Wall -Wextra -pthread

# Detect CPU features
HAVE_SHA := $(shell grep -c sha_ni /proc/cpuinfo 2>/dev/null || echo 0)
HAVE_AVX2 := $(shell grep -c avx2 /proc/cpuinfo 2>/dev/null || echo 0)
HAVE_SSE41 := $(shell grep -c sse4_1 /proc/cpuinfo 2>/dev/null || echo 0)

# Add SIMD flags based on detection
ifneq ($(HAVE_SHA),0)
    CXXFLAGS += -msha -DHAVE_SHA_NI
endif

ifneq ($(HAVE_AVX2),0)
    CXXFLAGS += -mavx2 -DHAVE_AVX2
endif

ifneq ($(HAVE_SSE41),0)
    CXXFLAGS += -msse4.1 -DHAVE_SSE41
endif

# Native optimization (comment out for portable build)
CXXFLAGS += -march=native

# Source files
SRCDIR := src
SOURCES := $(SRCDIR)/main.cpp $(SRCDIR)/stratum.cpp $(SRCDIR)/miner.cpp
HEADERS := $(SRCDIR)/sha256.hpp $(SRCDIR)/stratum.hpp $(SRCDIR)/miner.hpp

# Output
TARGET := btcminer

.PHONY: all clean install test

all: $(TARGET)

# Test binary
test_miner: $(SRCDIR)/test.cpp $(SRCDIR)/sha256.hpp
	$(CXX) $(CXXFLAGS) -o $@ $(SRCDIR)/test.cpp
	@echo "Test binary built: test_miner"

test: test_miner
	./test_miner

$(TARGET): $(SOURCES) $(HEADERS)
	$(CXX) $(CXXFLAGS) -o $@ $(SOURCES)
	@echo "Build complete: $(TARGET)"
	@echo "CPU features detected:"
	@[ $(HAVE_SHA) -ne 0 ] && echo "  - SHA-NI: enabled" || echo "  - SHA-NI: disabled"
	@[ $(HAVE_AVX2) -ne 0 ] && echo "  - AVX2: enabled" || echo "  - AVX2: disabled"
	@[ $(HAVE_SSE41) -ne 0 ] && echo "  - SSE4.1: enabled" || echo "  - SSE4.1: disabled"

clean:
	rm -f $(TARGET) test_miner

install: $(TARGET)
	install -m 755 $(TARGET) /usr/local/bin/

# Debug build
debug: CXXFLAGS := -std=c++17 -g -O0 -Wall -Wextra -pthread -DDEBUG
debug: $(TARGET)

# Portable build (no -march=native)
portable: CXXFLAGS := -std=c++17 -O3 -Wall -Wextra -pthread -msse4.1
portable: $(TARGET)
