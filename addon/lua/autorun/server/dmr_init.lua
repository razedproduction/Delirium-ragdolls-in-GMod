-- The clients' part: the DMR window, the fitted ragdolls' poses and the rig view.
AddCSLuaFile("autorun/client/dmr_client.lua")
AddCSLuaFile("dmr/cl_menu.lua")
AddCSLuaFile("dmr/cl_ragdolls.lua")
AddCSLuaFile("dmr/cl_rig_view.lua")

-- This addon's release: the module has to be the same one (scripts\package.ps1 checks both are Cargo.toml's).
DMR_ADDON_VERSION = "0.1.0"
-- The IPhysicsNM GMod's ragdolls need: native bodies (docs/adr/0026, 0027).
local INTERFACE = 6

-- What's wrong, to superadmins as they join (the console has it already).
local function tellSuperadmins(name, lines)
	hook.Add("PlayerInitialSpawn", name, function(ply)
		if not ply:IsSuperAdmin() then return end
		for _, l in ipairs(lines) do ply:ChatPrint(l) end
		ply:ChatPrint("[DMR] How to fix it: docs/install.md (FAQ)")
	end)
end

if not util.IsBinaryModuleInstalled("dmr") then
	local message = "[DMR] gmsv_dmr isn't installed: put gmsv_dmr_win64.dll into garrysmod/lua/bin (x86-64 branch only)"
	ErrorNoHalt(message .. "\n")
	tellSuperadmins("DMR_NoModule", { message })
	return
end

-- Once a Lua state: this file runs again on lua refresh, with the module and its agents kept (docs/adr/0021).
if not DMR then require("dmr") end

-- The module and this Lua from different releases: their natives and tables needn't match, so nothing runs.
if DMR.Version ~= DMR_ADDON_VERSION then
	local message = string.format("[DMR] gmsv_dmr_win64.dll is %s but the Lua addon is %s: DMR is disabled: the "
		.. "Workshop's addon and DMR-<version>-win64.zip (install.bat) have to be the same release", tostring(DMR.Version), DMR_ADDON_VERSION)
	ErrorNoHalt(message .. "\n")
	tellSuperadmins("DMR_Version", { message })
	return
end

-- The step callback goes before the physics environment does.
hook.Add("ShutDown", "DMR_Shutdown", function()
	DMR.Shutdown()
end)

-- Text to a player's console (the server's for nil): in pieces, as a user message carries 255 bytes at most.
function DMR_Say(ply, text)
	if not IsValid(ply) then
		print(text)
		return
	end
	for line in (text .. "\n"):gmatch("(.-)\n") do
		repeat
			local piece = line:sub(1, 200)
			-- Not in the middle of a UTF-8 character.
			while #piece < #line and #piece > 1 and line:byte(#piece + 1) >= 0x80 and line:byte(#piece + 1) < 0xC0 do
				piece = piece:sub(1, -2)
			end
			ply:PrintMessage(HUD_PRINTCONSOLE, piece)
			line = line:sub(#piece + 1)
		until line == ""
	end
end

include("dmr/debug_commands.lua")
include("dmr/nm_commands.lua")
include("dmr/follow.lua")
include("dmr/world.lua")
include("dmr/reactions.lua")
include("dmr/api.lua")
include("dmr/lifecycle.lua")
include("dmr/tuning.lua")
include("dmr/bench_scene.lua")
include("dmr/dev.lua")
include("dmr/skeletons.lua")
include("dmr/rig_view.lua")
include("dmr/ragdolls.lua")
include("dmr/deaths.lua")
include("dmr/live.lua")

-- What keeps DMR from working here, a line each (empty: nothing): the fork, NM's data. For the console at start,
-- superadmins as they join, and the DMR window's Debug tab.
function DMR_Problems()
	local out = {}
	local iface = DMR.InterfaceVersion()
	if not iface then
		-- docs/adr/0005: without IPhysicsNM nothing runs.
		out[#out + 1] = "vphysics.dll isn't VPhysics-Box3D (no IPhysicsNM): DMR is disabled. Steam's file check puts "
			.. "the original back: reinstall the fork (install.bat)"
	else
		local n = tonumber(iface:match("(%d+)$")) or 0
		if n < INTERFACE then
			out[#out + 1] = string.format("vphysics.dll is an older VPhysics-Box3D (%s, DMR needs VBOX3D_NM_%03d): reinstall "
				.. "the fork from this release (install.bat)", iface, INTERFACE)
		end
	end
	if DMR_GameDir() == "" then
		out[#out + 1] = "No GTA IV folder for NM's data: dmr_game_dir \"<your GTA IV folder>\" (or install.bat)"
	end
	return out
end

do
	local problems = DMR_Problems()
	if #problems > 0 then
		for i, p in ipairs(problems) do
			problems[i] = "[DMR] " .. p
			MsgC(Color(255, 120, 90), problems[i] .. "\n")
		end
		tellSuperadmins("DMR_Problems", problems)
	end
end
