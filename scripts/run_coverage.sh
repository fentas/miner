#!/bin/bash
# Run test coverage for btcminer
# Requires: cargo-tarpaulin (cargo install cargo-tarpaulin)

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${YELLOW}Running test coverage...${NC}"

# Check if tarpaulin is installed
if ! command -v cargo-tarpaulin &> /dev/null; then
    echo -e "${YELLOW}Installing cargo-tarpaulin...${NC}"
    cargo install cargo-tarpaulin
fi

# Create coverage directory
mkdir -p coverage

# Run coverage with tarpaulin
cargo tarpaulin \
    --out Html \
    --out Lcov \
    --output-dir coverage \
    --skip-clean

echo -e "\n${GREEN}Coverage report generated:${NC}"
echo -e "  HTML: coverage/tarpaulin-report.html"
echo -e "  LCOV: coverage/lcov.info"

# Print summary
echo -e "\n${GREEN}Coverage Summary:${NC}"
cargo tarpaulin --print-rust-flags --skip-clean 2>/dev/null || true

# Open in browser if available
if command -v xdg-open &> /dev/null; then
    echo -e "${YELLOW}Opening report in browser...${NC}"
    xdg-open coverage/tarpaulin-report.html 2>/dev/null &
fi
