[CmdletBinding()]
param(
    [string]$Binary,
    [string]$CacheSeed = (Join-Path $env:LOCALAPPDATA 'league-analysis\cache'),
    [ValidateSet('bans', 'blind', 'picks', 'build', 'explore', 'settings', 'ingame')]
    [string[]]$Views = @('bans', 'blind', 'picks', 'build', 'explore', 'settings', 'ingame')
)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
if (-not $Binary) {
    $Binary = Join-Path $repo 'target\debug\lol-companion.exe'
}
$Binary = (Resolve-Path -LiteralPath $Binary).Path
$config = Join-Path $repo 'config.toml'
$output = Join-Path $repo 'docs\views'
$workspace = Join-Path $repo 'build\readme-preview'
$appData = Join-Path $workspace 'league-analysis'
$cache = Join-Path $appData 'cache'
$cacheMarker = Join-Path $appData 'cache-seeded'
$run = Join-Path $workspace ('capture-' + [guid]::NewGuid().ToString('N'))
$null = New-Item -ItemType Directory -Path $cache, $output, $run -Force

if (-not (Test-Path -LiteralPath $cacheMarker -PathType Leaf)) {
    $seed = (Resolve-Path -LiteralPath $CacheSeed).Path
    Get-ChildItem -LiteralPath $seed -Force | ForEach-Object {
        Copy-Item -LiteralPath $_.FullName -Destination $cache -Recurse -Force
    }
    $null = New-Item -ItemType File -Path $cacheMarker
}

function Write-DraftReplay {
    param([string]$View, [string]$Destination)

    $allyChampions = @(122, 64, 61, 222, 89)
    $enemyChampions = @(36, 234, 103, 51, 111)
    $positions = @('top', 'jungle', 'middle', 'bottom', 'support')
    $allies = for ($slot = 0; $slot -lt $allyChampions.Count; $slot++) {
        $champion = $allyChampions[$slot]
        $picked = $champion
        $intent = 0
        if ($View -eq 'bans') {
            $picked = 0
            $intent = $champion
        } elseif ($slot -eq 4 -and $View -ne 'build') {
            $picked = 0
        }
        @{
            cellId = $slot
            assignedPosition = $positions[$slot]
            championId = $picked
            championPickIntent = $intent
        }
    }
    $enemies = for ($slot = 0; $slot -lt $enemyChampions.Count; $slot++) {
        $picked = $enemyChampions[$slot]
        if ($View -eq 'bans' -or ($View -eq 'blind' -and $slot -ge 3)) {
            $picked = 0
        }
        @{ cellId = $slot + 5; championId = $picked }
    }
    $action = @{
        actorCellId = 4
        type = 'pick'
        championId = $allies[4].championId
        completed = ($View -eq 'build')
        isInProgress = ($View -ne 'build')
    }
    $phase = 'PICK'
    if ($View -eq 'bans') {
        $action.type = 'ban'
        $phase = 'PLANNING'
    }
    $entry = @{
        session = @{
            localPlayerCellId = 4
            myTeam = @($allies)
            theirTeam = @($enemies)
            bans = @{ myTeamBans = @(238, 777); theirTeamBans = @(157, 53) }
            timer = @{ phase = $phase }
            actions = ,@($action)
        }
    }
    $entry | ConvertTo-Json -Depth 8 -Compress | Set-Content -LiteralPath $Destination -Encoding ASCII
}

$emptyReplay = Join-Path $run 'empty.jsonl'
$null = New-Item -ItemType File -Path $emptyReplay
$originalAppData = $env:LOCALAPPDATA
try {
    $env:LOCALAPPDATA = $workspace
    $selectedViews = $Views | ForEach-Object { $_.ToLowerInvariant() } | Select-Object -Unique
    foreach ($view in $selectedViews) {
        $screenshot = Join-Path $run "$view.png"
        if ($view -eq 'ingame') {
            $arguments = @(
                '--simulate', '--role', 'support', '--players', '--items', '3158,3109',
                '--champion', 'Leona', '--enemies', 'nautilus,caitlyn,ahri,viego,drmundo'
            )
        } else {
            $replay = $emptyReplay
            if ($view -ne 'explore' -and $view -ne 'settings') {
                $replay = Join-Path $run "$view.jsonl"
                Write-DraftReplay -View $view -Destination $replay
            }
            $arguments = @('--replay', $replay)
            if ($view -eq 'settings') {
                $arguments += '--screenshot-settings'
            }
        }
        $arguments += @('--screenshot', $screenshot, '--config', $config)
        $commandLine = ($arguments | ForEach-Object { '"{0}"' -f $_ }) -join ' '
        $stdout = Join-Path $run "$view.stdout.log"
        $stderr = Join-Path $run "$view.stderr.log"
        $process = Start-Process -FilePath $Binary -ArgumentList $commandLine -WorkingDirectory $repo `
            -WindowStyle Hidden -PassThru -RedirectStandardOutput $stdout -RedirectStandardError $stderr
        try {
            # Retain the native handle so PowerShell can read the exit code after the app closes.
            $null = $process.Handle
            $timer = [System.Diagnostics.Stopwatch]::StartNew()
            while (-not $process.WaitForExit(1000)) {
                if ($timer.Elapsed.TotalSeconds -ge 90) {
                    throw "Capture for $view timed out. See $stdout and $stderr."
                }
            }
            $process.WaitForExit()
            $process.Refresh()
            if ($process.ExitCode -ne 0 -or -not (Test-Path -LiteralPath $screenshot -PathType Leaf)) {
                throw "Capture for $view failed (exit $($process.ExitCode)). See $stdout and $stderr."
            }
            Copy-Item -LiteralPath $screenshot -Destination (Join-Path $output "$view.png") -Force
            Write-Output "Captured docs/views/$view.png"
        } finally {
            try {
                if (-not $process.HasExited) {
                    $process.Kill()
                }
            } finally {
                $process.Dispose()
            }
        }
    }
} finally {
    $env:LOCALAPPDATA = $originalAppData
}
