#!/usr/bin/env bash

# Fix clippy uninlined_format_args warnings
# This script fixes all the format string issues automatically

echo "🔧 Fixing clippy uninlined_format_args warnings..."

# Function to fix format strings in a file
fix_file() {
    local file="$1"
    echo "Fixing $file..."
    
    # Use sed to fix common patterns
    sed -i 's/format!(" | {}", \([^)]*\))/format!(" | {\1}")/g' "$file"
    sed -i 's/format!("{}{}", \([^,]*\), \([^)]*\))/format!("{\1}{\2}")/g' "$file"
    sed -i 's/format!("{} - {}", \([^,]*\), \([^)]*\))/format!("{\1} - {\2}")/g' "$file"
    sed -i 's/format!("{}", \([^)]*\))/format!("{\1}")/g' "$file"
    sed -i 's/format!("{:.1}", \([^)]*\))/format!("{\1:.1}")/g' "$file"
    sed -i 's/format!("{:.2}", \([^)]*\))/format!("{\1:.2}")/g' "$file"
    sed -i 's/format!("{:?}", \([^)]*\))/format!("{\1:?}")/g' "$file"
    sed -i 's/write!(f, "{}", \([^)]*\))/write!(f, "{\1}")/g' "$file"
    sed -i 's/write!(f, "{:?}", \([^)]*\))/write!(f, "{\1:?}")/g' "$file"
    sed -i 's/write!(f, "<{:?}>", \([^)]*\))/write!(f, "<{\1:?}>")/g' "$file"
    sed -i 's/write!(f, "<Alt+{}>", \([^)]*\))/write!(f, "<Alt+{\1}>")/g' "$file"
    sed -i 's/write!(f, "<Ctrl+{}>", \([^)]*\))/write!(f, "<Ctrl+{\1}>")/g' "$file"
    sed -i 's/panic!("unknown function key: F{}", \([^)]*\))/panic!("unknown function key: F{\1}")/g' "$file"
    sed -i 's/eprintln!("Error streaming logs for {}: {}", \([^,]*\), \([^)]*\))/eprintln!("Error streaming logs for {\1}: {\2}")/g' "$file"
}

echo "Processing Rust source files..."

# Fix all Rust files
find src -name "*.rs" -type f | while read -r file; do
    fix_file "$file"
done

echo "✅ Basic format string fixes applied!"
echo "🔧 Now applying specific multi-line format fixes..."

# Fix specific multi-line formats that sed can't handle easily
# We'll do these manually with targeted edits
