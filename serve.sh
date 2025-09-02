#!/bin/bash

# RegelRecht MkDocs Development Server
echo "🚀 Starting RegelRecht development server..."

# Check if mkdocs-material is installed
if ! command -v mkdocs &> /dev/null; then
    echo "❌ MkDocs not found. Installing..."
    pip install mkdocs-material mkdocs-blog-plugin
fi

# Start the development server
echo "📝 Starting MkDocs development server..."
echo "🌐 Opening http://127.0.0.1:8000"

# Start server and open browser
mkdocs serve --dev-addr=127.0.0.1:8000 &
SERVER_PID=$!

# Wait a moment for server to start
sleep 2

# Open browser (works on macOS, Linux, Windows)
if command -v open &> /dev/null; then
    open http://127.0.0.1:8000
elif command -v xdg-open &> /dev/null; then
    xdg-open http://127.0.0.1:8000
elif command -v start &> /dev/null; then
    start http://127.0.0.1:8000
else
    echo "🌐 Open http://127.0.0.1:8000 in your browser"
fi

# Wait for server process
wait $SERVER_PID