$ErrorActionPreference = "Stop"

$MarketplaceRepo = "naejin/monet-plugins"
$MarketplaceName = "monet-plugins"
$PluginName = "ecko"

function Write-Info($msg) { Write-Host "ecko: $msg" }
function Write-Ok($msg) { Write-Host "ecko: $msg" -ForegroundColor Green }
function Write-Warn($msg) { Write-Host "ecko: $msg" -ForegroundColor Yellow }
function Write-Err($msg) { Write-Host "error: $msg" -ForegroundColor Red }

# Parse arguments
$WithTools = $false
$ToolsOnly = $false
$PythonOnly = $false
$NodeOnly = $false

foreach ($arg in $args) {
    switch ($arg) {
        "--with-tools"  { $WithTools = $true }
        "--tools-only"  { $ToolsOnly = $true; $WithTools = $true }
        "--python-only" { $PythonOnly = $true; $WithTools = $true }
        "--node-only"   { $NodeOnly = $true; $WithTools = $true }
        { $_ -in "-h", "--help" } {
            Write-Host "Usage: install.ps1 [options]"
            Write-Host ""
            Write-Host "Options:"
            Write-Host "  --with-tools    Also install external tools (ruff, black, biome, etc.)"
            Write-Host "  --tools-only    Only install external tools (skip plugin install)"
            Write-Host "  --python-only   Only install Python tools"
            Write-Host "  --node-only     Only install Node.js tools"
            Write-Host "  -h, --help      Show this help"
            exit 0
        }
        default {
            Write-Err "Unknown option: $arg"
            exit 1
        }
    }
}

# --- Plugin Installation ---
if (-not $ToolsOnly) {
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

    Write-Ok "Plugin installed!"
}

# --- External Tools Installation ---
if ($WithTools) {
    Write-Host ""
    Write-Info "Installing external tools..."
    Write-Host ""

    # Detect Python package manager
    $PipCmd = $null
    if (-not $NodeOnly) {
        if (Get-Command uv -ErrorAction SilentlyContinue) {
            $PipCmd = "uv"
            $PipArgs = @("tool", "install")
            Write-Info "Using uv for Python tools"
        } elseif (Get-Command pipx -ErrorAction SilentlyContinue) {
            $PipCmd = "pipx"
            $PipArgs = @("install")
            Write-Info "Using pipx for Python tools"
        } elseif (Get-Command pip -ErrorAction SilentlyContinue) {
            $PipCmd = "pip"
            $PipArgs = @("install", "--user")
            Write-Info "Using pip for Python tools"
        } else {
            Write-Warn "No Python package manager found (uv, pipx, pip). Skipping Python tools."
        }
    }

    # Detect Node package manager
    $NpmCmd = $null
    if (-not $PythonOnly) {
        if (Get-Command npm -ErrorAction SilentlyContinue) {
            $NpmCmd = "npm"
            $NpmArgs = @("install", "-g")
            Write-Info "Using npm for Node tools"
        } elseif (Get-Command pnpm -ErrorAction SilentlyContinue) {
            $NpmCmd = "pnpm"
            $NpmArgs = @("add", "-g")
            Write-Info "Using pnpm for Node tools"
        } else {
            Write-Warn "No Node package manager found (npm, pnpm). Skipping Node tools."
        }
    }

    Write-Host ""

    # Python tools
    $PythonTools = @("black", "isort", "ruff", "pyright", "vulture")
    if ($PipCmd) {
        foreach ($tool in $PythonTools) {
            if (Get-Command $tool -ErrorAction SilentlyContinue) {
                Write-Host "  ✓ $tool (already installed)" -ForegroundColor Green
            } else {
                Write-Host -NoNewline "  ○ $tool..."
                try {
                    & $PipCmd @PipArgs $tool 2>$null | Out-Null
                    Write-Host "`r  ✓ $tool" -ForegroundColor Green
                } catch {
                    Write-Host "`r  ✗ $tool (install failed)" -ForegroundColor Red
                }
            }
        }
    }

    # Node tools
    $NodeTools = @(
        @{ Package = "prettier"; Bin = "prettier" },
        @{ Package = "@biomejs/biome"; Bin = "biome" },
        @{ Package = "typescript"; Bin = "tsc" }
    )
    if ($NpmCmd) {
        foreach ($tool in $NodeTools) {
            if (Get-Command $tool.Bin -ErrorAction SilentlyContinue) {
                Write-Host "  ✓ $($tool.Package) (already installed)" -ForegroundColor Green
            } else {
                Write-Host -NoNewline "  ○ $($tool.Package)..."
                try {
                    & $NpmCmd @NpmArgs $tool.Package 2>$null | Out-Null
                    Write-Host "`r  ✓ $($tool.Package)" -ForegroundColor Green
                } catch {
                    Write-Host "`r  ✗ $($tool.Package) (install failed)" -ForegroundColor Red
                }
            }
        }
    }

    # knip
    if (Get-Command npx -ErrorAction SilentlyContinue) {
        Write-Host "  ✓ knip (runs via npx)" -ForegroundColor Green
    }

    Write-Host ""
    Write-Ok "Tools setup complete!"
}

Write-Host ""
Write-Info "Restart Claude Code to start using ecko."
