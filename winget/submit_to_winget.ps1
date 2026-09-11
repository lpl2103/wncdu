<#
.SYNOPSIS
  Submete o pacote WNCDU para o repositório oficial microsoft/winget-pkgs.
.DESCRIPTION
  Faz o fork do microsoft/winget-pkgs usando gh CLI, cria branch, copia os manifestos e abre o Pull Request oficial.
#>

param(
    [string]$Version = "0.1.0"
)

$ErrorActionPreference = "Stop"

Write-Host "=== Submissao do WNCDU v$Version para o Winget ===" -ForegroundColor Cyan

# Verificar gh CLI autenticado
$ghUser = gh api user --jq .login
if (-not $ghUser) {
    Write-Error "gh CLI nao esta autenticado. Execute 'gh auth login' primeiro."
    exit 1
}
Write-Host "Usuario GitHub autenticado: $ghUser" -ForegroundColor Green

$ManifestSrc = "$PSScriptRoot/manifests/l/lpl2103/wncdu/$Version"
if (-not (Test-Path $ManifestSrc)) {
    Write-Error "Pasta de manifestos nao encontrada: $ManifestSrc"
    exit 1
}

# Criar fork se ainda nao existir
Write-Host "Verificando/Criando fork de microsoft/winget-pkgs..." -ForegroundColor Yellow
gh repo fork microsoft/winget-pkgs --clone=false --default-branch-only 2>$null

$TempDir = Join-Path ([System.IO.Path]::GetTempPath()) "winget-pkgs-$([System.Guid]::NewGuid().ToString().Substring(0,8))"
Write-Host "Clonando fork temporariamente em $TempDir..." -ForegroundColor Yellow

git clone --depth 1 "https://github.com/$ghUser/winget-pkgs.git" $TempDir
Push-Location $TempDir

try {
    $BranchName = "wncdu-$Version"
    git checkout -b $BranchName

    $DestDir = "manifests/l/lpl2103/wncdu/$Version"
    New-Item -ItemType Directory -Force -Path $DestDir | Out-Null
    Copy-Item "$ManifestSrc/*" -Destination $DestDir -Force

    git add $DestDir
    git commit -m "New package: lpl2103.wncdu version $Version"
    git push -u origin $BranchName

    Write-Host "Criando Pull Request para microsoft/winget-pkgs..." -ForegroundColor Cyan
    $PrTitle = "New package: lpl2103.wncdu version $Version"
    $PrBody = @"
## Package Submission: lpl2103.wncdu v$Version

- **Package Name**: WNCDU
- **Description**: Fast disk usage analyzer with TUI for Windows, inspired by ncdu
- **License**: MIT
- **Release URL**: https://github.com/lpl2103/wncdu/releases/tag/v$Version
"@

    gh pr create --repo microsoft/winget-pkgs --head "$ghUser`:$BranchName" --title $PrTitle --body $PrBody
    Write-Host "PR submetido com sucesso para microsoft/winget-pkgs!" -ForegroundColor Green
}
finally {
    Pop-Location
    if (Test-Path $TempDir) {
        Remove-Item -Recurse -Force $TempDir -ErrorAction SilentlyContinue
    }
}
