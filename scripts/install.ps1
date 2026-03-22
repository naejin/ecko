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

# Ensure binary is available (download/build on first run)
$pluginDir = $null
$hasPy = Get-Command python3 -ErrorAction SilentlyContinue
if ($hasPy) {
    $pluginDir = & python3 -c @"
import json, pathlib
config_dir = pathlib.Path.home() / '.claude'
plugins_file = config_dir / 'plugins.json'
if plugins_file.exists():
    for p in json.load(open(plugins_file)):
        if p.get('name') == 'ecko':
            print(p.get('directory', ''))
            break
"@ 2>$null
}

if ($pluginDir -and (Test-Path "$pluginDir\scripts\run.cmd")) {
    Write-Info "Downloading ecko binary..."
    $out = & "$pluginDir\scripts\run.cmd" --version 2>$null
    if ($LASTEXITCODE -eq 0) {
        Write-Info "Binary ready."
    } else {
        Write-Info "Binary download skipped. It will be fetched on first use."
    }
}

Write-Host ""
Write-Info "Ecko installed!"
Write-Info "No external tools needed - ecko v2 checks are native."
Write-Info "Optional: install pyright, tsc, golangci-lint, or clippy for deep analysis."
Write-Info "Restart Claude Code to start using ecko."
Write-Host ""
