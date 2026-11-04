# Builds the module and packs the release: dist\DMR-<version>-win64.zip (docs\install.md says what's in it).
#
#   .\scripts\package.ps1             build the module and pack (the fork: the golden checkout's build)
#   .\scripts\package.ps1 -NoBuild    pack what's built
#
# The version is Cargo.toml's (workspace.package); the Lua addon's DMR_ADDON_VERSION has to be the same (the addon is
# the Workshop's, published from addon\: docs/adr/0035). Nothing of GTA IV goes in (docs/adr/0006). The fork's
# vphysics.dll and its licences come from the golden checkout next to this one (..\DMS in Gmod, docs/adr/0036), built
# there by its scripts\build-vphysics.ps1.

param(
	[switch]$NoBuild
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$golden = Join-Path (Split-Path -Parent $root) 'DMS in Gmod'

$cargo = Get-Content (Join-Path $root 'Cargo.toml') -Raw
if ($cargo -notmatch '(?ms)\[workspace\.package\].*?^version\s*=\s*"([^"]+)"') { throw 'no version in Cargo.toml' }
$version = $Matches[1]
$init = Get-Content (Join-Path $root 'addon\lua\autorun\server\dmr_init.lua') -Raw
if ($init -notmatch 'DMR_ADDON_VERSION\s*=\s*"([^"]+)"') { throw 'no DMR_ADDON_VERSION in dmr_init.lua' }
if ($Matches[1] -ne $version) { throw "dmr_init.lua's DMR_ADDON_VERSION is $($Matches[1]), Cargo.toml's version $version" }

if (-not $NoBuild) {
	& (Join-Path $PSScriptRoot 'build.ps1')
}

$bins = [ordered]@{
	'gmsv_dmr_win64.dll' = Join-Path $root 'dist\gmsv_dmr_win64.dll'
	'vphysics.dll' = Join-Path $golden 'vphysics\build\windows-x64\src\Release\vphysics.dll'
}
foreach ($b in $bins.Values) { if (-not (Test-Path $b)) { throw "$b isn't built" } }

# No AVX2/FMA the CPU isn't asked about: none in our Rust (its ymm code is the runtime-dispatched kind in std and
# memchr); FMA only in the C runtime's maths (sin, cos, atan2_fma, __remainder_piby2_fma3, …), which the UCRT itself
# takes only on a CPU with FMA3 (__use_fma3_lib); in the fork, only Box3D's wide8.c (/arch:AVX2), which runs only when
# b3HasAVX2 says so. Read with the build's .pdb next to it, so the instructions have their functions' names.
$dumpbin = $null
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
if (Test-Path $vswhere) {
	$dumpbin = & $vswhere -latest -products * -find 'VC\Tools\MSVC\**\bin\Hostx64\x64\dumpbin.exe' | Select-Object -First 1
}
$crtMaths = '^(_?_?(sin|cos|tan|asin|acos|atan2?|exp|log|pow|fma)f?(_fma3?)?|__remainder_piby2.*)$'
$withPdb = @{
	'gmsv_dmr_win64.dll' = Join-Path $root 'target\x86_64-pc-windows-msvc\release\gmsv_dmr.dll'
}
if ($dumpbin) {
	foreach ($name in $withPdb.Keys) {
		$fn = ''
		$bad = @{}
		foreach ($l in (& $dumpbin /nologo /disasm:nobytes $withPdb[$name])) {
			if ($l -match '^([^\s].*):$') { $fn = $Matches[1] }
			elseif ($l -match '\bvf(n?)m(add|sub)' -and $fn -notmatch $crtMaths) { $bad[$fn] = 1 + $bad[$fn] }
		}
		if ($bad.Count -gt 0) {
			throw "$name has FMA outside the C runtime's maths ($(($bad.Keys | Select-Object -First 5) -join ', ')): something was built for a newer CPU (no target-cpu, CLAUDE.md)"
		}
		$crt = & $dumpbin /nologo /dependents $bins[$name] | Select-String -Pattern '(?i)vcruntime|msvcp'
		if ($crt) { throw "$name needs $($crt.Line.Trim()): the C runtime has to be linked in (.cargo\config.toml)" }
	}
	Write-Host 'checked: gmsv_dmr_win64.dll: FMA only in the C runtime''s dispatched maths, no VCRUNTIME'
} else {
	Write-Warning 'dumpbin not found: the binaries were not checked for FMA and the C runtime'
}

$name = "DMR-$version-win64"
$stage = Join-Path $root "dist\$name"
if (Test-Path $stage) { Remove-Item $stage -Recurse -Force -Confirm:$false }
New-Item -ItemType Directory -Force (Join-Path $stage 'bin'), (Join-Path $stage 'licenses') | Out-Null
foreach ($b in $bins.GetEnumerator()) { Copy-Item $b.Value (Join-Path $stage "bin\$($b.Key)") }
# The Lua addon isn't packed: it's the Workshop's (docs/adr/0035), published from addon\ by hand. The installer is a
# .bat with its version put in, in CRLF (cmd finds labels wrongly in an LF file) and ASCII.
$bat = (Get-Content (Join-Path $PSScriptRoot 'release\install.bat') -Raw) -replace '\r?\n', "`r`n"
if ($bat -notmatch '@VERSION@') { throw 'install.bat has no @VERSION@' }
if ($bat -match '[^\x00-\x7F]') { throw 'install.bat has to be ASCII' }
[IO.File]::WriteAllText((Join-Path $stage 'install.bat'), $bat.Replace('@VERSION@', $version), [Text.Encoding]::ASCII)
Copy-Item (Join-Path $root 'docs\install.md') $stage
Copy-Item (Join-Path $root 'LICENSE') $stage
Copy-Item (Join-Path $root 'THIRD_PARTY.md') $stage
$notes = Join-Path $root "docs\release-notes-$version.md"
if (Test-Path $notes) { Copy-Item $notes (Join-Path $stage 'RELEASE-NOTES.md') }
# vphysics.dll's parts keep their licences (THIRD_PARTY.md); the Source SDK's asks for its LICENSE and
# thirdpartylegalnotices.txt with every copy.
$licences = [ordered]@{
	'VPhysics-Box3D.txt' = 'vphysics\LICENSE'
	'Box3D.txt' = 'vphysics\external\box3d\LICENSE'
	'qhull.txt' = 'vphysics\external\qhull\COPYING.txt'
	'SourceSDK2013-LICENSE.txt' = 'vphysics\external\source-sdk-2013\LICENSE'
	'SourceSDK2013-thirdpartylegalnotices.txt' = 'vphysics\external\source-sdk-2013\thirdpartylegalnotices.txt'
}
foreach ($l in $licences.GetEnumerator()) {
	$from = Join-Path $golden $l.Value
	if (-not (Test-Path $from)) { throw "$from not found (git submodule update --init?)" }
	Copy-Item $from (Join-Path $stage "licenses\$($l.Key)")
}
# What the player runs: no development leftovers (the bench's settings, golden data).
$extra = Get-ChildItem $stage -Recurse -File | Where-Object { $_.Name -match '(?i)bench\.txt|\.rec$|game_dir' }
if ($extra) { throw "not for a release: $($extra.FullName -join ', ')" }

$zip = Join-Path $root "dist\$name.zip"
if (Test-Path $zip) { Remove-Item $zip -Confirm:$false }
Compress-Archive -Path $stage -DestinationPath $zip
$size = [math]::Round((Get-Item $zip).Length / 1MB, 1)
Write-Host "-> $zip ($size MB)"
