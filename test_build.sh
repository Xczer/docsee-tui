#!/bin/bash

echo "🔨 Testing Docsee compilation..."
cd /home/xczer/Documents/Projects/docsee

echo "📝 Running cargo check first..."
cargo check

if [ $? -eq 0 ]; then
    echo "✅ Cargo check passed! Now running full build..."
    cargo build
    
    if [ $? -eq 0 ]; then
        echo "🎉 Build successful! Docsee compiled without errors."
        echo ""
        echo "🚀 You can now run your application with:"
        echo "   cargo run"
        echo ""
        echo "📋 The fixes applied:"
        echo "   1. Fixed memory_stats field access (not Optional)"
        echo "   2. Fixed network rx_bytes/tx_bytes (not Optional)" 
        echo "   3. Fixed blkio_stats field access (not Optional)"
        echo "   4. Fixed pids_stats.current access (not Optional)"
    else
        echo "❌ Build failed. Please check the error messages above."
    fi
else
    echo "❌ Cargo check failed. Please fix the errors above before building."
fi
