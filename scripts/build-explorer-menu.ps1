# Builds the Windows 11 File Explorer menu: maxrar_shell.dll plus the sparse
# package (maxrar-explorer-menu.msix) that the NSIS installer registers.
#
#   -Sign   sign both with Azure Artifact Signing (needs AZURE_* and trusted-signing-cli).
#           The package publisher is read from the DLL's signing certificate.
#   without -Sign the package is unsigned. Windows will not install it from the
#           .msix; register target/explorer-menu/layout with Developer Mode instead
#           (see docs/DESKTOP-INTEGRATION.md).
param(
    [string]$Target = 'x86_64-pc-windows-msvc',
    [switch]$Sign
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$targetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $root 'target' }
$out = Join-Path $root 'target/explorer-menu'
$layout = Join-Path $out 'layout'
$unsignedPublisher = 'CN=MaxRAR Development'

function Invoke-Sign([string]$file) {
    trusted-signing-cli -e https://eus.codesigning.azure.net -a matily-signing -c matily-public -d MaxRAR $file | Out-Host
    if ($LASTEXITCODE -ne 0) { throw "Signing failed: $file" }
    $sig = Get-AuthenticodeSignature -LiteralPath $file
    if ($sig.Status -ne 'Valid') { throw "Signature is $($sig.Status): $file" }
    $sig
}

cargo build -p maxrar-shell --release --locked --target $Target
if ($LASTEXITCODE -ne 0) { throw 'maxrar-shell build failed' }

Remove-Item -Recurse -Force $out -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $layout | Out-Null
$dll = Join-Path $out 'maxrar_shell.dll'
Copy-Item (Join-Path $targetDir "$Target/release/maxrar_shell.dll") $dll

$publisher = $unsignedPublisher
if ($Sign) { $publisher = (Invoke-Sign $dll).SignerCertificate.Subject }

$version = (Get-Content -Raw (Join-Path $root 'src-tauri/tauri.conf.json') | ConvertFrom-Json).version
if ($version -notmatch '^\d+\.\d+\.\d+$') { throw "Unsupported version for the package: $version" }
$package = Join-Path $root 'src-tauri/shell/windows/package'
Copy-Item -Recurse (Join-Path $package 'Assets') $layout
$manifest = (Get-Content -Raw (Join-Path $package 'AppxManifest.xml')).
    Replace('__PUBLISHER__', [Security.SecurityElement]::Escape($publisher)).
    Replace('__VERSION__', "$version.0")
[IO.File]::WriteAllText((Join-Path $layout 'AppxManifest.xml'), $manifest, [Text.UTF8Encoding]::new($false))

$kits = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits/10/bin'
$makeappx = Get-ChildItem $kits -Filter makeappx.exe -Recurse -ErrorAction SilentlyContinue |
    Where-Object { $_.Directory.Name -eq 'x64' } | Sort-Object FullName -Descending | Select-Object -First 1
if (!$makeappx) { throw 'makeappx.exe not found; install the Windows SDK' }
$msix = Join-Path $out 'maxrar-explorer-menu.msix'
& $makeappx.FullName pack /d $layout /p $msix /nv /o | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'makeappx failed' }
if ($Sign) { Invoke-Sign $msix | Out-Null }

Write-Output "Explorer menu ready ($publisher): $out"
