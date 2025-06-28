#!/usr/bin/env bash

# Quick fix for the most common clippy uninlined_format_args issues
# This targets the specific patterns found in the error log

echo "🔧 Applying targeted fixes for clippy uninlined_format_args..."

# Function to apply sed replacements to a file
fix_patterns() {
    local file="$1"
    echo "  Fixing $file..."
    
    # Common single variable patterns
    sed -i 's/format!("\([^"]*\){}", \([^)]*\))/format!("\1{\2}")/g' "$file"
    sed -i 's/format!("\([^"]*\){:.1}", \([^)]*\))/format!("\1{\2:.1}")/g' "$file"
    sed -i 's/format!("\([^"]*\){:.2}", \([^)]*\))/format!("\1{\2:.2}")/g' "$file"
    sed -i 's/format!("\([^"]*\){:?}", \([^)]*\))/format!("\1{\2:?}")/g' "$file"
    
    # Two variable patterns
    sed -i 's/format!("\([^"]*\){} - {}", \([^,]*\), \([^)]*\))/format!("\1{\2} - {\3}")/g' "$file"
    sed -i 's/format!("\([^"]*\){}{}", \([^,]*\), \([^)]*\))/format!("\1{\2}{\3}")/g' "$file"
    
    # Error message patterns
    sed -i 's/format!("Failed to \([^"]*\) {}", \([^)]*\))/format!("Failed to \1 {\2}")/g' "$file"
    sed -i 's/format!("Error \([^"]*\): {}", \([^)]*\))/format!("Error \1: {\2}")/g' "$file"
    sed -i 's/format!("\([^"]*\) '\''{}'\''", \([^)]*\))/format!("\1 '\''{}\2'\''")/g' "$file"
    
    # Write macro patterns
    sed -i 's/write!(f, "{}", \([^)]*\))/write!(f, "{\1}")/g' "$file"
    sed -i 's/write!(f, "{:?}", \([^)]*\))/write!(f, "{\1:?}")/g' "$file"
    sed -i 's/write!(f, "<{:?}>", \([^)]*\))/write!(f, "<{\1:?}>")/g' "$file"
    sed -i 's/write!(f, "<Alt+{}>", \([^)]*\))/write!(f, "<Alt+{\1}>")/g' "$file"
    sed -i 's/write!(f, "<Ctrl+{}>", \([^)]*\))/write!(f, "<Ctrl+{\1}>")/g' "$file"
    
    # Panic macro patterns  
    sed -i 's/panic!("unknown function key: F{}", \([^)]*\))/panic!("unknown function key: F{\1}")/g' "$file"
    
    # eprintln patterns
    sed -i 's/eprintln!("\([^"]*\){}: {}", \([^,]*\), \([^)]*\))/eprintln!("\1{\2}: {\3}")/g' "$file"
}

# List of files that need fixing based on the error log
files_to_fix=(
    "src/app.rs"
    "src/docker/containers.rs"
    "src/docker/images.rs"
    "src/docker/networks.rs"
    "src/docker/volumes.rs"
    "src/events/key.rs"
    "src/ui/containers.rs"
    "src/ui/images.rs"
    "src/ui/networks.rs"
    "src/ui/volumes.rs"
    "src/ui/logs_viewer.rs"
    "src/ui/search_filter.rs"
    "src/ui/shell_executor.rs"
    "src/ui/stats_viewer.rs"
)

# Apply fixes to each file
for file in "${files_to_fix[@]}"; do
    if [ -f "$file" ]; then
        fix_patterns "$file"
    else
        echo "  Warning: $file not found"
    fi
done

echo "✅ Applied targeted fixes!"
echo "🧪 Now test with: cargo clippy --all-targets --all-features -- -D warnings"
