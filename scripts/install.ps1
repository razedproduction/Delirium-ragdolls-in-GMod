# Installs the built module and the Lua addon into Garry's Mod, beside the golden project's (gmsv_dms, addons\dms):
#   dist\gmsv_dmr_win64.dll -> garrysmod\lua\bin
#   addon\                  -> garrysmod\addons\dmr (replaced as a whole)
#
#   .\scripts\install.ps1                          GMod from Steam's default library
#   .\scripts\install.ps1 -GarrysMod "D:\...\GarrysMod"
#   .\scripts\install.ps1 -Fork [-Vphysics <dll>]  also the VPhysics-Box3D fork -> bin\win64\vphysics.dll: the golden
#                                                  checkout's build (..\DMS in Gmod\vphysics\build, its
#                                                  scripts\build-vphysics.ps1; docs\adr\0036) or -Vphysics; Steam's own
#                                                  is kept once as vphysics.dll.original

param(
	[string]$GarrysMod,
	[string]$Vphysics,
	[switch]$Fork
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
# The golden checkout next to this one (read only): the fork is built and held to the Playroom there.
$golden = Join-Path (Split-Path -Parent $root) 'DMS in Gmod'

if (-not $GarrysMod) {
	$steam = (Get-ItemProperty 'HKCU:\Software\Valve\Steam' -ErrorAction SilentlyContinue).SteamPath
	if ($steam) { $GarrysMod = Join-Path $steam 'steamapps\common\GarrysMod' }
}
if (-not $GarrysMod -or -not (Test-Path (Join-Path $GarrysMod 'garrysmod'))) {
	throw "Garry's Mod not found, pass -GarrysMod <path to the GarrysMod folder>"
}
$target = $GarrysMod

$dll = Join-Path $root 'dist\gmsv_dmr_win64.dll'
if (-not (Test-Path $dll)) { throw "$dll not built yet, run scripts\build.ps1" }

$bin = Join-Path $target 'garrysmod\lua\bin'
New-Item -ItemType Directory -Force $bin | Out-Null
Copy-Item $dll $bin -Force
Write-Host "-> $bin\gmsv_dmr_win64.dll"

$addon = Join-Path $target 'garrysmod\addons\dmr'
if (Test-Path $addon) { Remove-Item $addon -Recurse -Force -Confirm:$false }
Copy-Item (Join-Path $root 'addon') $addon -Recurse
Write-Host "-> $addon"

if ($Fork) {
	$win64 = Join-Path $target 'bin\win64'
	$original = Join-Path $win64 'vphysics.dll.original'
	if (-not $Vphysics) {
		$Vphysics = Join-Path $golden 'vphysics\build\windows-x64\src\Release\vphysics.dll'
		if (-not (Test-Path $Vphysics)) { throw "$Vphysics not built yet: scripts\build-vphysics.ps1 in the golden checkout" }
	}
	if (-not (Test-Path $Vphysics)) { throw "$Vphysics not found" }
	# Only the first install keeps what is there: later ones would back up the fork itself.
	if (-not (Test-Path $original)) { Copy-Item (Join-Path $win64 'vphysics.dll') $original }
	Copy-Item $Vphysics (Join-Path $win64 'vphysics.dll') -Force
	Write-Host "-> $win64\vphysics.dll (VPhysics-Box3D from $Vphysics; the original: vphysics.dll.original)"
}
