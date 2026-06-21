Write-Host "Setting up Git hooks for Pollen DEK..." -ForegroundColor Cyan

# Set core.hooksPath to the .githooks directory
git config core.hooksPath .githooks

Write-Host "✅ Git hooks configured successfully!" -ForegroundColor Green
Write-Host "Pre-push hook will now run automatically before every 'git push'." -ForegroundColor Yellow
