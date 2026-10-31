param([string]$Out='j201a.png')
$ff='F:\d1\<pc>-import\d1xenia\local\DEVLOGS\tools\imageio_ffmpeg\binaries\ffmpeg-win-x86_64-v7.1.exe'
$v=(Get-ChildItem F:\d1\<pc>-import\d1xenia\local\RPCS3_FINAL124_SMOKE001\recordings\BLES01857 | Where-Object Length -gt 0 | Sort-Object LastWriteTime | Select-Object -Last 1)
"$($v.Name) $($v.LastWriteTime.ToString('HH:mm:ss')) now $(Get-Date -Format HH:mm:ss)"
& $ff -hide_banner -loglevel error -y -sseof -3 -i $v.FullName -update 1 -vf scale=640:-1 (Join-Path $PSScriptRoot $Out)

