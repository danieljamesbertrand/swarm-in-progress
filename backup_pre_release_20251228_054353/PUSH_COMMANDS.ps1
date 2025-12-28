# PowerShell script to push to new repository
# Run this after creating the repository on GitHub/GitLab

$newRepoName = "16-node-burst-with-jsons-and-web-console"
$githubUsername = "danieljamesbertrand"  # Change this to your username

Write-Host "=== Pushing to New Repository ===" -ForegroundColor Green
Write-Host ""

# Remove old remote
Write-Host "Removing old remote..." -ForegroundColor Yellow
git remote remove origin

# Add new remote (HTTPS)
$newRemote = "https://github.com/$githubUsername/$newRepoName.git"
Write-Host "Adding new remote: $newRemote" -ForegroundColor Yellow
git remote add origin $newRemote

# Push to new repository
Write-Host "Pushing to new repository..." -ForegroundColor Yellow
git push -u origin main

Write-Host ""
Write-Host "✅ Done! Repository pushed to:" -ForegroundColor Green
Write-Host "   https://github.com/$githubUsername/$newRepoName" -ForegroundColor Cyan
Write-Host ""
Write-Host "If you prefer SSH, use:" -ForegroundColor Gray
Write-Host "   git remote set-url origin git@github.com:$githubUsername/$newRepoName.git" -ForegroundColor Gray













