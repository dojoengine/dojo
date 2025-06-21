#!/bin/bash
set -e

echo "🧪 Testing dojoup update implementation..."

# Make executable
chmod +x dojoup/dojoup

# Test help command
echo "✅ Testing help command:"
./dojoup/dojoup update --help

echo -e "\n✅ Testing main help includes update:"
./dojoup/dojoup --help | grep "update" || echo "❌ Update not found in help"

echo -e "\n✅ Testing function existence:"
grep -q "update_dojo()" dojoup/dojoup && echo "✅ update_dojo function found"
grep -q "usage_update()" dojoup/dojoup && echo "✅ usage_update function found" 
grep -q "update)" dojoup/dojoup && echo "✅ update case found in main"

echo -e "\n🎉 Basic validation complete!"
