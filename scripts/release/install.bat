@echo off
rem DMR installer for players: double-click it in the unpacked release (DMR-<version>-win64\), Garry's Mod closed.
rem
rem What goes where (install.md):
rem   bin\vphysics.dll        -> GarrysMod\bin\win64\vphysics.dll (Steam's own kept once as vphysics.dll.original)
rem   bin\gmsv_dmr_win64.dll  -> GarrysMod\garrysmod\lua\bin
rem   your GTA IV folder      -> GarrysMod\garrysmod\data\dmr\game_dir.txt (the path only: NM's data is read from it)
rem The Lua addon isn't in the release: it's the Workshop's (WORKSHOP below). Nothing of GTA IV is copied anywhere,
rem nothing leaves your machine. DMR goes beside the Delirium Playroom (DMS) if that's installed: both use the same
rem vphysics.dll.
rem
rem No ( ) blocks with paths in them: "Program Files (x86)" would end a block; one-line ifs and gotos only.

setlocal EnableExtensions DisableDelayedExpansion
title DMR installer
cd /d "%~dp0"
set "HERE=%~dp0"
rem scripts\package.ps1 puts the release's version here.
set "VERSION=@VERSION@"
rem The DMR addon's Workshop item (not the Playroom's 3829811829); empty until it's published.
set "WORKSHOP_ID="
set "WORKSHOP=https://steamcommunity.com/sharedfiles/filedetails/?id=%WORKSHOP_ID%"
rem UTF-8, so a GTA IV folder with non-Latin letters reaches game_dir.txt as DMR reads it; the old code page at the end.
rem chcp reads stdin away when it's redirected: <nul.
set "OLDCP="
for /f "tokens=2 delims=:" %%C in ('chcp ^<nul') do set "OLDCP=%%C"
chcp 65001 <nul >nul
rem Answers asked again at most this many times in all (a closed input would ask for ever).
set "TRIES=0"

echo ==========================================================
echo   DMR %VERSION% for Garry's Mod (64-bit, Windows)
echo   Delirium (NM) on GMod's ragdolls, NPCs and Workshop models
echo ==========================================================
echo.
echo DMR needs two things from you:
echo   - the DMR addon from the Steam Workshop (the Lua part)
echo   - your own copy of GTA IV: NM's data is read from it.
echo.

:ask_action
set /a TRIES+=1
if %TRIES% GTR 10 goto no_answer
echo   1. Install or update DMR (and set your GTA IV folder)
echo   2. Set your GTA IV folder only
echo   3. Uninstall DMR
echo.
set "ACTION="
set /p "ACTION=Choose 1, 2 or 3: "
if "%ACTION%"=="1" goto get_gmod
if "%ACTION%"=="2" goto get_gmod
if "%ACTION%"=="3" goto get_gmod
echo Type a number from 1 to 3.
echo.
goto ask_action

rem ---- Garry's Mod: Steam's own library, then the other ones (libraryfolders.vdf), then asked.
:get_gmod
echo.
set "GMOD="
set "STEAM="
for /f "tokens=2,*" %%A in ('reg query "HKCU\Software\Valve\Steam" /v SteamPath 2^>nul ^| find /i "SteamPath"') do set "STEAM=%%B"
if not defined STEAM goto ask_gmod
set "STEAM=%STEAM:/=\%"
call :try_library "%STEAM%"
set "VDF=%STEAM%\steamapps\libraryfolders.vdf"
if not defined GMOD if exist "%VDF%" for /f "usebackq tokens=1,*" %%A in ("%VDF%") do if /i "%%~A"=="path" call :try_library %%B
if not defined GMOD goto ask_gmod
echo Garry's Mod found: %GMOD%
set "ANSWER="
set /p "ANSWER=Use this folder? [Y/n]: "
if /i "%ANSWER%"=="n" goto ask_gmod
goto check_gmod

:ask_gmod
set /a TRIES+=1
if %TRIES% GTR 10 goto no_answer
echo Garry's Mod's folder (the one with garrysmod\ and bin\ in it; you can drag it onto this window),
echo for example D:\SteamLibrary\steamapps\common\GarrysMod
set "GMOD="
set /p "GMOD=Garry's Mod folder: "
if not defined GMOD goto ask_gmod
call :clean_path GMOD

:check_gmod
if not exist "%GMOD%\garrysmod\" echo ERROR: no garrysmod folder in %GMOD%& goto ask_gmod
set "WIN64=%GMOD%\bin\win64"
set "GAMEDATA=%GMOD%\garrysmod"
set "ORIGINAL=%WIN64%\vphysics.dll.original"
if not exist "%WIN64%\vphysics.dll" goto no_x64
tasklist /fo csv /nh 2>nul | findstr /i /b /c:"\"gmod" /c:"\"hl2.exe" >nul
if not errorlevel 1 echo ERROR: Garry's Mod is running: close it first (its vphysics.dll can't be replaced while it runs).& goto fail
if "%ACTION%"=="3" goto uninstall
if "%ACTION%"=="2" goto gta
goto install

:no_x64
echo ERROR: %WIN64%\vphysics.dll isn't there: DMR needs 64-bit Garry's Mod (its bin\win64 folder);
echo let Steam update the game, or check this is the GarrysMod folder.
goto fail

rem ---- The fork and the module.
:install
echo.
for %%F in (vphysics.dll gmsv_dmr_win64.dll) do if not exist "%HERE%bin\%%F" echo ERROR: bin\%%F is missing: unpack the whole zip.& goto fail
rem Only the first install keeps what is there: later ones would back up the fork itself.
if exist "%ORIGINAL%" goto copy_fork
copy /y "%WIN64%\vphysics.dll" "%ORIGINAL%" >nul
if errorlevel 1 echo ERROR: can't back up %WIN64%\vphysics.dll (try "Run as administrator").& goto fail
:copy_fork
copy /y "%HERE%bin\vphysics.dll" "%WIN64%\vphysics.dll" >nul
if errorlevel 1 echo ERROR: can't write %WIN64%\vphysics.dll (try "Run as administrator").& goto fail
echo -^> %WIN64%\vphysics.dll (VPhysics-Box3D with IPhysicsNM; Steam's own: vphysics.dll.original)
if not exist "%GAMEDATA%\lua\bin\" mkdir "%GAMEDATA%\lua\bin"
copy /y "%HERE%bin\gmsv_dmr_win64.dll" "%GAMEDATA%\lua\bin\gmsv_dmr_win64.dll" >nul
if errorlevel 1 echo ERROR: can't write %GAMEDATA%\lua\bin\gmsv_dmr_win64.dll.& goto fail
echo -^> %GAMEDATA%\lua\bin\gmsv_dmr_win64.dll

rem The Workshop's downloads are in the library's steamapps\workshop\content\4000 (Garry's Mod is app 4000).
if not defined WORKSHOP_ID echo The DMR addon isn't on the Workshop yet: put its addon\ folder into %GAMEDATA%\addons\dmr.& goto gta
if exist "%GMOD%\..\..\workshop\content\4000\%WORKSHOP_ID%\" echo The DMR addon is subscribed on the Workshop.& goto gta
echo.
echo The DMR addon (its Lua) isn't downloaded from the Workshop yet: subscribe to it, or DMR won't start.
echo   %WORKSHOP%
set "ANSWER="
set /p "ANSWER=Open its Workshop page in Steam? [Y/n]: "
if /i not "%ANSWER%"=="n" start "" "steam://url/CommunityFilePage/%WORKSHOP_ID%"

rem ---- The player's GTA IV folder: NM's data is read from it in the game.
:gta
echo.
set "GTA="
if exist "%GAMEDATA%\data\dmr\game_dir.txt" set /p GTA=<"%GAMEDATA%\data\dmr\game_dir.txt"
rem The Playroom's (DMS), if it knows the folder already.
if not defined GTA if exist "%GAMEDATA%\data\dms\game_dir.txt" set /p GTA=<"%GAMEDATA%\data\dms\game_dir.txt"
if not defined GTA goto ask_gta
echo GTA IV folder: %GTA%
set "ANSWER="
set /p "ANSWER=Use this folder? [Y/n]: "
if /i "%ANSWER%"=="n" goto ask_gta
goto check_gta

:ask_gta
set /a TRIES+=1
if %TRIES% GTR 10 goto no_answer
echo Your GTA IV folder: the PC game's (with GTAIV.exe) or an Xbox 360 copy's (with default.xex),
echo for example D:\Games\Grand Theft Auto IV
set "GTA="
set /p "GTA=GTA IV folder: "
if not defined GTA goto ask_gta
call :clean_path GTA

:check_gta
if exist "%GTA%\GTAIV.exe" goto gta_ok
if exist "%GTA%\default.xex" goto gta_ok
echo ERROR: %GTA% isn't a GTA IV folder (no GTAIV.exe or default.xex in it).
goto ask_gta
:gta_ok
if not exist "%GAMEDATA%\data\dmr\" mkdir "%GAMEDATA%\data\dmr"
> "%GAMEDATA%\data\dmr\game_dir.txt" echo %GTA%
echo -^> %GAMEDATA%\data\dmr\game_dir.txt (DMR reads NM's data from %GTA%; dmr_game_dir overrides it)
goto done

rem ---- The module out; Steam's vphysics.dll back unless the Playroom (DMS) still needs the fork.
:uninstall
echo.
echo The DMR module is removed (garrysmod\data\dmr is kept). Steam's vphysics.dll goes back unless DMS is installed.
set "ANSWER="
set /p "ANSWER=Uninstall DMR? [y/N]: "
if /i not "%ANSWER%"=="y" echo Nothing changed.& goto done
if exist "%GAMEDATA%\lua\bin\gmsv_dmr_win64.dll" del /q "%GAMEDATA%\lua\bin\gmsv_dmr_win64.dll"
echo removed %GAMEDATA%\lua\bin\gmsv_dmr_win64.dll
if exist "%GAMEDATA%\addons\dmr\" rmdir /s /q "%GAMEDATA%\addons\dmr"
if exist "%GAMEDATA%\lua\bin\gmsv_dms_win64.dll" echo vphysics.dll left as it is: the Delirium Playroom (DMS) uses it too.& goto unsubscribe
if not exist "%ORIGINAL%" goto no_original
copy /y "%ORIGINAL%" "%WIN64%\vphysics.dll" >nul
if errorlevel 1 echo ERROR: can't write %WIN64%\vphysics.dll (try "Run as administrator").& goto fail
del /q "%ORIGINAL%"
echo -^> %WIN64%\vphysics.dll: Steam's own again
goto unsubscribe
:no_original
echo No vphysics.dll.original: vphysics.dll left as it is (Steam's "Verify integrity of game files" puts its own back).
:unsubscribe
if defined WORKSHOP_ID echo Unsubscribe from the DMR addon on the Workshop to remove its Lua too: %WORKSHOP%
set "CODE=0"
goto quit

:done
echo.
echo Done. In Garry's Mod (a local game, 64-bit): dmr_ragdoll in the console spawns a citizen with NM,
echo dmr_menu opens the DMR window.
set "CODE=0"
goto quit

:no_answer
echo.
echo No answer: run install.bat again.
set "CODE=1"
goto quit

:fail
echo.
echo Stopped: nothing after the error was done.
set "CODE=1"

:quit
echo.
pause
if defined OLDCP chcp %OLDCP% >nul
exit /b %CODE%

rem ---- Helpers.

rem A Steam library (its path as libraryfolders.vdf writes it, \\ for \): Garry's Mod there, if it's the first found.
:try_library
if defined GMOD exit /b 0
set "LIB=%~1"
set "LIB=%LIB:\\=\%"
if exist "%LIB%\steamapps\common\GarrysMod\garrysmod\" set "GMOD=%LIB%\steamapps\common\GarrysMod"
exit /b 0

rem A typed or dragged path: no quotes, no trailing \ (it would escape the closing quote of "%X%" for an exe).
:clean_path
call set "P=%%%1%%"
set "P=%P:"=%"
if "%P:~-1%"=="\" set "P=%P:~0,-1%"
set "%1=%P%"
exit /b 0
