param([int]$Hours = 6)
# Keep the display on while a capture/automation session runs (the same request a video player makes:
# SetThreadExecutionState with ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED). No power
# settings are changed; the request ends when this process exits.
Add-Type -Namespace D1 -Name Power -MemberDefinition '[DllImport("kernel32.dll")] public static extern uint SetThreadExecutionState(uint f);'
[void][D1.Power]::SetThreadExecutionState([uint32]2147483651)   # 0x80000003
Start-Sleep -Seconds ($Hours * 3600)
