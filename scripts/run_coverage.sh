#!/bin/bash
# Run tests with coverage report

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Build directory
BUILD_DIR="${BUILD_DIR:-build_coverage}"

echo -e "${YELLOW}Building with coverage...${NC}"

# Create build directory
mkdir -p "$BUILD_DIR"
cd "$BUILD_DIR"

# Configure with coverage
cmake .. \
    -DCMAKE_BUILD_TYPE=Debug \
    -DBUILD_TESTS=ON \
    -DENABLE_COVERAGE=ON \
    -DNATIVE_BUILD=ON

# Build
make -j$(nproc)

echo -e "\n${YELLOW}Running tests...${NC}"

# Clear previous coverage
lcov --directory . --zerocounters 2>/dev/null || true

# Run tests
./btcminer_test || true

echo -e "\n${YELLOW}Generating coverage report...${NC}"

# Capture coverage
lcov --directory . --capture --output-file coverage.info

# Remove system files and test files from coverage
lcov --remove coverage.info '/usr/*' '*/tests/*' --output-file coverage.info

# Generate HTML report
genhtml coverage.info --output-directory coverage_html

# Print summary
echo -e "\n${GREEN}Coverage Summary:${NC}"
lcov --summary coverage.info

echo -e "\n${GREEN}Coverage report generated: ${BUILD_DIR}/coverage_html/index.html${NC}"

# Open in browser if available
if command -v xdg-open &> /dev/null; then
    echo -e "${YELLOW}Opening report in browser...${NC}"
    xdg-open coverage_html/index.html 2>/dev/null &
fi
