# Build/run environment for the d1preservation (PS4) tree on Windows.
# Dot-source it in PowerShell:  . F:\d1\tools\d1-env.ps1
$pkgs = "$env:LOCALAPPDATA\Microsoft\WinGet\Packages"
$ninja = (Get-ChildItem $pkgs -Recurse -Filter ninja.exe -ErrorAction SilentlyContinue | Select-Object -First 1).DirectoryName
$env:PROTOC = (Get-ChildItem $pkgs -Recurse -Filter protoc.exe -ErrorAction SilentlyContinue | Select-Object -First 1).FullName
$env:LIBCLANG_PATH = 'F:\d1\tools\llvm-23.1.3\bin'
$env:RUSTUP_HOME = 'F:\d1\tools\rust\rustup'
$env:CARGO_HOME = 'F:\d1\tools\rust\cargo'
$env:Path = "F:\d1\tools\rust\cargo\bin;F:\d1\tools\llvm-23.1.3\bin;C:\Program Files\CMake\bin;$ninja;$env:Path"
$env:PYTHONPATH = "$env:USERPROFILE\.local\lib\d1-tools"
$env:D1_REPO = 'F:\d1\d1preservation-main'
$env:SHADPS4_BIN = 'F:\d1\tools\shadPS4\0.19.0\shadPS4.exe'
"d1 env ready: cargo $(cargo --version), protoc $env:PROTOC, shadPS4 $env:SHADPS4_BIN"
