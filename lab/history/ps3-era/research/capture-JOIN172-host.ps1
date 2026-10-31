$ErrorActionPreference='Stop'
$rt='F:\d1\<pc>-import\d1xenia\local'
. 'F:\d1\d1xenia\scripts\Final124RunHelpers.ps1'
$logs=Join-Path $rt 'SIGNON_LOCAL\logs'
$identity=(Read-D1ProcessManifest (Join-Path $logs 'FINAL124_JOIN172-processes.json')).processes | Where-Object tag -EQ 'rpcs3'
$client=Get-D1OwnedProcess $identity
if (-not $client) { throw 'Exact client is already closed; no dump captured' }
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class D1HostDump172 {
    [DllImport("DbgHelp.dll", SetLastError=true)]
    public static extern bool MiniDumpWriteDump(IntPtr process, uint pid, IntPtr file, uint type, IntPtr exception, IntPtr user, IntPtr callback);
    [DllImport("kernel32.dll", SetLastError=true)]
    public static extern IntPtr OpenThread(uint access, bool inherit, uint tid);
    [DllImport("kernel32.dll")]
    public static extern uint GetProcessIdOfThread(IntPtr thread);
    [DllImport("kernel32.dll")]
    public static extern int GetThreadDescription(IntPtr thread, out IntPtr name);
    [DllImport("kernel32.dll")]
    public static extern IntPtr LocalFree(IntPtr value);
    [DllImport("kernel32.dll")]
    public static extern bool CloseHandle(IntPtr handle);
}
'@
$names=@()
foreach ($thread in $client.Threads) {
    $handle=[D1HostDump172]::OpenThread(0x0800,$false,[uint32]$thread.Id)
    if ($handle -eq [IntPtr]::Zero) { continue }
    try {
        if ([D1HostDump172]::GetProcessIdOfThread($handle) -ne $client.Id) { continue }
        $name=[IntPtr]::Zero
        if ([D1HostDump172]::GetThreadDescription($handle,[ref]$name) -eq 0) {
            try { $names+=@{id=$thread.Id;name=[Runtime.InteropServices.Marshal]::PtrToStringUni($name)} }
            finally { if ($name -ne [IntPtr]::Zero) { [D1HostDump172]::LocalFree($name) | Out-Null } }
        }
    } finally { [D1HostDump172]::CloseHandle($handle) | Out-Null }
}
$path=Join-Path $rt 'RESEARCH\JOIN172-host-stall.dmp'
$file=[IO.File]::Open($path,[IO.FileMode]::CreateNew,[IO.FileAccess]::ReadWrite,[IO.FileShare]::None)
$at=[DateTime]::UtcNow.ToString('o')
try {
    if (-not (Get-D1OwnedProcess $identity)) { throw 'Client closed before host capture' }
    $ok=[D1HostDump172]::MiniDumpWriteDump($client.Handle,[uint32]$client.Id,$file.SafeFileHandle.DangerousGetHandle(),0x1000,[IntPtr]::Zero,[IntPtr]::Zero,[IntPtr]::Zero)
    $errorCode=[Runtime.InteropServices.Marshal]::GetLastWin32Error()
} finally { $file.Dispose() }
[ordered]@{at=$at;finished_at=[DateTime]::UtcNow.ToString('o');client=$identity;dump=$path;success=$ok;win32_error=$errorCode;bytes=(Get-Item -LiteralPath $path).Length;threads=$names;
    scope='Private host diagnostic capture after configured sampling interval. MiniDumpNormal + thread information; no full-memory dump. Capture can perturb timing; no performance or clean native comparison claim.'} |
    ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $logs 'JOIN172-host-capture.json') -Encoding utf8
if (-not $ok) { throw "Host dump failed ($errorCode); native run remains owned by original wrapper" }
'Private host dump saved'
