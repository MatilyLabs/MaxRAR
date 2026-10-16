$ErrorActionPreference = 'Stop'
$installer = Get-ChildItem target/release/bundle/nsis/*.exe | Select-Object -First 1
if (!$installer) { throw 'NSIS installer missing' }
$install = Join-Path $env:LOCALAPPDATA 'MaxRAR Shell CI'
$process = Start-Process $installer.FullName -ArgumentList @('/S',"/D=$install") -Wait -PassThru
if ($process.ExitCode -ne 0) { throw 'Installer failed' }
$exe = Join-Path $install 'maxrar.exe'
foreach ($entry in @(@('*','MaxRAR.Compress','compress'),@('Directory','MaxRAR.Compress','compress'),@('SystemFileAssociations\.zip','MaxRAR.Extract','extract'),@('SystemFileAssociations\.rar','MaxRAR.Extract','extract'))) {
    $key = "Registry::HKEY_CURRENT_USER\Software\Classes\$($entry[0])\shell\$($entry[1])\command"
    $value = (Get-Item -LiteralPath $key).GetValue('')
    $expected = '"' + $exe + '" --' + $entry[2] + ' -- "%1"'
    if ($value -ne $expected) { throw "Wrong shell command: $value" }
}
$shortcut = Join-Path ([Environment]::GetFolderPath('SendTo')) 'Compress with MaxRAR.lnk'
if (!(Test-Path $shortcut)) { throw 'Send To shortcut missing' }
$null = Start-Process (Join-Path $install 'uninstall.exe') -ArgumentList '/S' -Wait -PassThru
for ($i=0; $i -lt 20; $i++) {
    if (!(Test-Path -LiteralPath 'Registry::HKEY_CURRENT_USER\Software\Classes\*\shell\MaxRAR.Compress')) { break }
    Start-Sleep -Seconds 1
}
if (Test-Path -LiteralPath 'Registry::HKEY_CURRENT_USER\Software\Classes\*\shell\MaxRAR.Compress') { throw 'Shell registration remained after uninstall' }
if (Test-Path $shortcut) { throw 'Send To shortcut remained after uninstall' }
if (Test-Path -LiteralPath 'Registry::HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\.rar\shell\MaxRAR.Extract') { throw 'RAR shell registration remained after uninstall' }
Write-Output 'Explorer registration, quoting, Send To and uninstall checks passed.'
