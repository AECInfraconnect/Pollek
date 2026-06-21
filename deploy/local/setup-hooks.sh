#!/usr/bin/env bash
set -e

echo "Setting up Git hooks for Pollen DEK..."
git config core.hooksPath .githooks

echo "Making hooks executable..."
chmod +x .githooks/*

echo "✅ Git hooks configured successfully!"
echo "Pre-push hook will now run automatically before every 'git push'."
