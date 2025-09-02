#!/bin/bash

# RegelRecht MkDocs Build Script
echo "🏗️  Building RegelRecht documentation..."

# Check if mkdocs-material is installed
if ! command -v mkdocs &> /dev/null; then
    echo "❌ MkDocs not found. Installing..."
    pip install mkdocs-material mkdocs-blog-plugin
fi

# Clean previous build
echo "🧹 Cleaning previous build..."
rm -rf site/

# Build the site
echo "📦 Building site..."
mkdocs build

if [ $? -eq 0 ]; then
    echo "✅ Build successful!"
    echo "📁 Site generated in ./site/"
    echo ""
    echo "To serve locally: ./serve.sh"
    echo "To deploy: Upload ./site/ to your web server"
else
    echo "❌ Build failed!"
    exit 1
fi