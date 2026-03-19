$ErrorActionPreference = "Stop"

$MarketplaceRepo = "naejin/monet-plugins"
$MarketplaceName = "monet-plugins"
$PluginName = "ecko"

function Write-Info($msg) { Write-Host "ecko: $msg" }
function Write-Err($msg) { Write-Host "error: $msg" -ForegroundColor Red }

# Require Claude Code
$ClaudePath = Get-Command claude -ErrorAction SilentlyContinue
if (-not $ClaudePath) {
    Write-Err "Claude Code not found on PATH."
    Write-Err "Install it first: https://docs.anthropic.com/en/docs/claude-code"
    Write-Err ""
    Write-Err "Then run this script again, or install manually:"
    Write-Err "  claude plugin marketplace add $MarketplaceRepo"
    Write-Err "  claude plugin install $PluginName@$MarketplaceName"
    exit 1
}

# Add marketplace if not already registered
$marketplaceList = & claude plugin marketplace list 2>$null
if ($marketplaceList -notmatch $MarketplaceName) {
    Write-Info "Adding marketplace..."
    & claude plugin marketplace add $MarketplaceRepo
    if ($LASTEXITCODE -ne 0) {
        Write-Err "Failed to add marketplace. Try manually:"
        Write-Err "  claude plugin marketplace add $MarketplaceRepo"
        exit 1
    }
}

# Install or update plugin
$pluginList = & claude plugin list 2>$null
if ($pluginList -match "$PluginName@$MarketplaceName") {
    Write-Info "Updating plugin..."
    & claude plugin marketplace update $MarketplaceName
    & claude plugin update "$PluginName@$MarketplaceName"
} else {
    Write-Info "Installing plugin..."
    & claude plugin install "$PluginName@$MarketplaceName"
    if ($LASTEXITCODE -ne 0) {
        Write-Err "Failed to install plugin. Try manually:"
        Write-Err "  claude plugin install $PluginName@$MarketplaceName"
        exit 1
    }
}

Write-Host ""
Write-Info "Ecko installed!"
Write-Info "Restart Claude Code to start using ecko."
Write-Host ""

# Check for tool runners
$hasUvx = Get-Command uvx -ErrorAction SilentlyContinue
$hasNpx = Get-Command npx -ErrorAction SilentlyContinue
if ($hasUvx -or $hasNpx) {
    Write-Info "External tools (ruff, biome, etc.) will run automatically via uvx/npx."
} else {
    Write-Info "Tip: install uv (https://docs.astral.sh/uv) or Node.js for full tool coverage."
    Write-Info "Ecko works without them - it just runs fewer checks."
}
