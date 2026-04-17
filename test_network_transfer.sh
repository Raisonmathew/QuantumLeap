#!/bin/bash
# End-to-end network transfer test script

set -e

echo "🧪 QLTP Network Transfer Test"
echo "================================"
echo ""

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Cleanup function
cleanup() {
    echo ""
    echo "🧹 Cleaning up..."
    if [ ! -z "$SERVER_PID" ]; then
        kill $SERVER_PID 2>/dev/null || true
    fi
    rm -rf test_transfer_temp
}

trap cleanup EXIT

# Create test directory
mkdir -p test_transfer_temp/send
mkdir -p test_transfer_temp/receive

# Create test file (10MB)
echo "📝 Creating test file (10MB)..."
dd if=/dev/urandom of=test_transfer_temp/send/test_file.bin bs=1M count=10 2>/dev/null
TEST_FILE_HASH=$(shasum -a 256 test_transfer_temp/send/test_file.bin | awk '{print $1}')
echo "   Original file hash: $TEST_FILE_HASH"
echo ""

# Build CLI
echo "🔨 Building CLI..."
cargo build --release -p qltp-cli --quiet
echo "   ✓ Build complete"
echo ""

# Start receiver in background
echo "📥 Starting receiver on 127.0.0.1:9999..."
./target/release/qltp receive -l 127.0.0.1:9999 -o test_transfer_temp/receive &
SERVER_PID=$!

# Wait for server to start
sleep 2

# Check if server is running
if ! kill -0 $SERVER_PID 2>/dev/null; then
    echo -e "${RED}✗ Server failed to start${NC}"
    exit 1
fi
echo "   ✓ Server started (PID: $SERVER_PID)"
echo ""

# Send file
echo "📤 Sending file..."
if ./target/release/qltp send test_transfer_temp/send/test_file.bin 127.0.0.1:9999; then
    echo -e "${GREEN}   ✓ File sent successfully${NC}"
else
    echo -e "${RED}   ✗ File send failed${NC}"
    exit 1
fi
echo ""

# Wait a moment for file to be written
sleep 1

# Verify received file
echo "🔍 Verifying received file..."
if [ -f test_transfer_temp/receive/test_file.bin ]; then
    RECEIVED_FILE_HASH=$(shasum -a 256 test_transfer_temp/receive/test_file.bin | awk '{print $1}')
    echo "   Received file hash: $RECEIVED_FILE_HASH"
    
    if [ "$TEST_FILE_HASH" = "$RECEIVED_FILE_HASH" ]; then
        echo -e "${GREEN}   ✓ File integrity verified!${NC}"
        echo ""
        echo -e "${GREEN}✅ All tests passed!${NC}"
        exit 0
    else
        echo -e "${RED}   ✗ File hash mismatch!${NC}"
        exit 1
    fi
else
    echo -e "${RED}   ✗ Received file not found${NC}"
    exit 1
fi

# Made with Bob
