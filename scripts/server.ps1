# The bench (docs\bench.md): Garry's Mod driven by tools\dmr-ctl.
#
#   .\scripts\server.ps1 start [-Map gm_flatgrass] [-GtaDir <GTA IV dir>] [-NoInstall] [-Hidden] [-Tickrate 60]
#   .\scripts\server.ps1 stop | restart | status
#
# It's the game itself in a 1280x720 window, so the test can be watched (-Hidden: -textmode, the window hidden), a
# listen server on 127.0.0.1: the only place VPhysics-Box3D runs (no dedicated server: the x86-64 one is built with
# newer vphysics interfaces than the fork).
#
# start installs dist\ and addon\ (install.ps1), writes garrysmod\data\dmr\bench.txt (dmr_game_dir, dmr_dev 1 and a new
# token; the addon reads and deletes it) and starts the game, RCON on 127.0.0.1 with a new password, the console
# logged to garrysmod\console.log. What dmr-ctl needs goes to .bench\server.txt (not in git).
# The game can't run twice: don't start the bench while playing (or while the golden checkout's bench runs), stop it
# before playing.

param(
	[Parameter(Position = 0)][ValidateSet('start', 'stop', 'restart', 'status')][string]$Action = 'status',
	[string]$GarrysMod,
	[string]$Map = 'gm_flatgrass',
	[string]$GtaDir,
	[int]$Port = 27115,
	# Not the golden bench's 27099: either can be up without the other noticing.
	[int]$DevPort = 27098,
	[int]$Wait = 180,
	[switch]$NoInstall,
	[switch]$Hidden,
	# The server's tick (66 by default): 60 makes a Source step NM's step (ROADMAP M4, docs\bench.md).
	[int]$Tickrate = 0
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$benchDir = Join-Path $root '.bench'
$stateFile = Join-Path $benchDir 'server.txt'
New-Item -ItemType Directory -Force $benchDir | Out-Null

function Read-State {
	$s = @{}
	if (Test-Path $stateFile) {
		foreach ($l in Get-Content $stateFile) {
			$k, $v = $l -split '=', 2
			if ($v -ne $null) { $s[$k.Trim()] = $v.Trim() }
		}
	}
	$s
}

function Get-GameDir {
	if ($GarrysMod) { return $GarrysMod }
	$steam = (Get-ItemProperty 'HKCU:\Software\Valve\Steam' -ErrorAction SilentlyContinue).SteamPath
	if ($steam) { return (Join-Path $steam 'steamapps\common\GarrysMod') }
	throw "Garry's Mod not found, pass -GarrysMod <path to the GarrysMod folder>"
}

function Get-Running($s) {
	if (-not $s.pid) { return $null }
	$p = Get-Process -Id $s.pid -ErrorAction SilentlyContinue
	if ($p -and $p.ProcessName -like 'gmod*') { return $p }
	return $null
}

function New-Secret {
	$bytes = New-Object byte[] 16
	[System.Security.Cryptography.RandomNumberGenerator]::Create().GetBytes($bytes)
	-join ($bytes | ForEach-Object { $_.ToString('x2') })
}

function Hide-Window([IntPtr]$h) {
	if (-not ('DmrBench.Win' -as [type])) {
		Add-Type -Name Win -Namespace DmrBench -MemberDefinition '[DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);'
	}
	[DmrBench.Win]::ShowWindow($h, 0) | Out-Null
	$true
}

function Test-Port([int]$p) {
	$c = New-Object System.Net.Sockets.TcpClient
	try {
		$t = $c.ConnectAsync('127.0.0.1', $p)
		return $t.Wait(300) -and $c.Connected
	} catch { return $false } finally { $c.Close() }
}

function Show-Status {
	$s = Read-State
	$p = Get-Running $s
	if (-not $p) {
		Write-Host "bench: not running"
		return
	}
	$rcon = if (Test-Port ([int]$s.rcon_port)) { 'up' } else { 'down' }
	$dev = if (Test-Port ([int]$s.dev_port)) { 'up' } else { 'down' }
	Write-Host ("bench: running, pid {0}, {1}; RCON 127.0.0.1:{2} {3}; dmr-ctl channel 127.0.0.1:{4} {5}" -f `
		$p.Id, $s.dir, $s.rcon_port, $rcon, $s.dev_port, $dev)
	Write-Host "log: $($s.log)"
}

function Stop-Bench {
	$s = Read-State
	$p = Get-Running $s
	if (-not $p) {
		Write-Host "bench: not running"
	} else {
		$ctl = Join-Path $root 'target\release\dmr-ctl.exe'
		if (Test-Path $ctl) { try { & $ctl exec quit 2>$null | Out-Null } catch {} }
		if (-not $p.WaitForExit(10000)) { Stop-Process -Id $p.Id -Force -Confirm:$false }
		Write-Host "bench: stopped (pid $($p.Id))"
	}
	if ($s.dir) {
		$settings = Join-Path $s.dir 'garrysmod\data\dmr\bench.txt'
		if (Test-Path $settings) { Remove-Item $settings -Force -Confirm:$false }
	}
	if (Test-Path $stateFile) { Remove-Item $stateFile -Force -Confirm:$false }
}

function Start-Bench {
	$s = Read-State
	if (Get-Running $s) {
		Write-Host "bench: already running (pid $($s.pid)); restart to start it anew"
		return
	}
	$dir = Get-GameDir
	$exe = Join-Path $dir 'gmod_win64.exe'
	if (-not (Test-Path $exe)) { throw "$exe not found (the x86-64 branch of Garry's Mod)" }
	if (Get-Process gmod*, hl2 -ErrorAction SilentlyContinue) { throw "Garry's Mod is running: close it first (it can't run twice)" }
	if (-not $NoInstall) { & (Join-Path $PSScriptRoot 'install.ps1') -GarrysMod $dir }

	# The GTA IV folder: -GtaDir, DMR_GAME_DIR, fit\game_dir.txt (dmr-check --game), else the golden checkout's.
	if (-not $GtaDir) { $GtaDir = $env:DMR_GAME_DIR }
	foreach ($f in @((Join-Path $root 'fit\game_dir.txt'), (Join-Path (Split-Path -Parent $root) 'DMS in Gmod\golden\game_dir.txt'))) {
		if (-not $GtaDir -and (Test-Path $f)) { $GtaDir = (Get-Content $f -Raw).Trim() }
	}
	if (-not $GtaDir) { Write-Host "no GTA IV folder (-GtaDir, DMR_GAME_DIR or fit\game_dir.txt): NM's data won't load" }

	$password = New-Secret
	$token = New-Secret
	$data = Join-Path $dir 'garrysmod\data\dmr'
	New-Item -ItemType Directory -Force $data | Out-Null
	[System.IO.File]::WriteAllLines((Join-Path $data 'bench.txt'), [string[]]@(
		"dmr_game_dir `"$GtaDir`""
		"dmr_dev_port $DevPort"
		"dmr_dev_token $token"
		"dmr_dev 1"
	))
	$log = Join-Path $dir 'garrysmod\console.log'
	if (Test-Path $log) { Remove-Item $log -Force -Confirm:$false }

	# A listen server (2 slots: maxplayers 1 would be single player, no RCON); -usercon opens RCON there.
	# Shown: a normal window, so the test can be watched; -Hidden: -textmode, the window hidden below.
	$view = if ($Hidden) { @('-textmode', '-w', '640', '-h', '480') } else { @('-w', '1280', '-h', '720') }
	$launch = $view + @('-nosound', '-novid', '-windowed', '-usercon', '-nocrashdialog', '-condebug', '-ip', '127.0.0.1',
		'-port', $Port, '+sv_lan', '1', '+sv_cheats', '1', '+rcon_password', $password)
	if ($Tickrate -gt 0) { $launch += @('-tickrate', $Tickrate) }
	$launch += @('+maxplayers', '2', '+sv_hibernate_think', '1', '+map', $Map)
	$style = if ($Hidden) { 'Hidden' } else { 'Normal' }
	$p = Start-Process -FilePath $exe -ArgumentList $launch -WorkingDirectory $dir -WindowStyle $style -PassThru
	[System.IO.File]::WriteAllLines($stateFile, [string[]]@(
		"kind=game"
		"dir=$dir"
		"pid=$($p.Id)"
		"rcon_port=$Port"
		"rcon_password=$password"
		"dev_port=$DevPort"
		"dev_token=$token"
		"log=$log"
		"map=$Map"
	))
	Write-Host "bench: starting $(Split-Path -Leaf $exe), pid $($p.Id), $Map, 127.0.0.1:$Port; waiting for the dmr-ctl channel..."
	$deadline = (Get-Date).AddSeconds($Wait)
	# Only -Hidden hides the game's window (it opens one even with -textmode and a hidden start).
	$hidden = -not $Hidden
	while ((Get-Date) -lt $deadline) {
		if ($p.HasExited) { throw "the bench exited ($($p.ExitCode)): see $log" }
		if (-not $hidden) {
			$p.Refresh()
			if ($p.MainWindowHandle -ne [IntPtr]::Zero) { $hidden = Hide-Window $p.MainWindowHandle }
		}
		if ($hidden -and (Test-Port $DevPort)) {
			Show-Status
			return
		}
		Start-Sleep -Milliseconds 500
	}
	Write-Host "the dmr-ctl channel didn't come up in $Wait s (see $log); RCON: $(if (Test-Port $Port) { 'up' } else { 'down' })"
}

switch ($Action) {
	'start' { Start-Bench }
	'stop' { Stop-Bench }
	'restart' { Stop-Bench; Start-Bench }
	'status' { Show-Status }
}
