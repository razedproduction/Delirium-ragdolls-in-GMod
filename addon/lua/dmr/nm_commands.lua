-- Delirium inside the module (ROADMAP M2): NM's data from the user's GTA IV, the self-test, reference lists.
-- Superadmins only. None of it needs IPhysicsNM: NM runs on its own body here.

local gameDir = CreateConVar("dmr_game_dir", "", FCVAR_ARCHIVE + FCVAR_PROTECTED,
	"Your GTA IV folder (PC: with GTAIV.exe; Xbox 360: with default.xex): NM's data is read from there")

local say = DMR_Say

-- The GTA IV folder: dmr_game_dir, else the one install.bat wrote to data/dmr/game_dir.txt (a path only, no
-- game files: docs/adr/0006, 0010). "" when there's neither.
function DMR_GameDir()
	local dir = gameDir:GetString()
	if dir ~= "" then return dir end
	return string.Trim(file.Read("dmr/game_dir.txt", "DATA") or "")
end

local function allowed(ply)
	return not IsValid(ply) or ply:IsSuperAdmin()
end

-- Loads NM's data from the GTA IV folder (once), or says what's missing and where to point it.
local function load(ply)
	local ok, info = pcall(DMR.LoadData, DMR_GameDir())
	if not ok then
		say(ply, "[DMR] NM's data isn't loaded: " .. tostring(info))
		return
	end
	return info
end

local function vec(v)
	if not v then return "-" end
	if v.w then return string.format("(%.4f, %.4f, %.4f, %.4f)", v.x, v.y, v.z, v.w) end
	return string.format("(%.4f, %.4f, %.4f)", v.x, v.y, v.z)
end

concommand.Add("dmr_data", function(ply)
	if not allowed(ply) then return end
	local d = load(ply)
	if not d then return end
	say(ply, string.format("[DMR] NM's data: %s (%s), Homer %d parts, %d surfaces (floor row %d), taskparams: %s",
		d.dir, d.platform, d.parts, d.surfaces, d.floorMaterial, d.taskparams))
end)

-- Homer in LEAD on a flat floor under bodyBalance for [seconds = 2]: he has to stay up.
concommand.Add("dmr_selftest", function(ply, _, args)
	if not allowed(ply) then return end
	if not load(ply) then return end
	local ok, r = pcall(DMR.SelfTest, tonumber(args[1]) or 2)
	if not ok then return say(ply, "[DMR] selftest: " .. tostring(r)) end
	say(ply, string.format("[DMR] selftest: %s", r.verdict))
	say(ply, string.format("  %d steps of 1/60 s; pelvis %.3f -> min %.3f -> %.3f m; head %.3f -> %.3f m; " ..
		"balancing %s, fallen %s", r.steps, r.pelvisStart, r.pelvisMin, r.pelvisEnd, r.headStart, r.headEnd,
		tostring(r.balancing), tostring(r.fallen)))
end)

concommand.Add("dmr_nm_parts", function(ply)
	if not allowed(ply) then return end
	for i, name in ipairs(DMR.PartNames()) do
		say(ply, string.format("  %2d %s", i - 1, name))
	end
end)

-- Every message, or [message]'s params.
concommand.Add("dmr_nm_messages", function(ply, _, args)
	if not allowed(ply) then return end
	if not args[1] then
		local names = DMR.Messages()
		say(ply, string.format("[DMR] %d messages (dmr_nm_messages <name> for its params):", #names))
		say(ply, "  " .. table.concat(names, " "))
		return
	end
	local ok, params = pcall(DMR.MessageParams, args[1])
	if not ok then return say(ply, "[DMR] " .. tostring(params)) end
	say(ply, string.format("[DMR] %s: %d params (* kept only, the port doesn't act on it)", args[1], #params))
	for _, p in ipairs(params) do
		local default = type(p.default) == "table" and vec(p.default) or tostring(p.default)
		say(ply, string.format("  %s%s %s = %s  %s", p.name, p.used and "" or "*", p.kind, default, p.doc))
	end
end)

concommand.Add("dmr_nm_build", function(ply)
	if not allowed(ply) then return end
	if not load(ply) then return end
	for _, p in ipairs(DMR.BuildParts()) do
		say(ply, string.format("  %2d %-16s parent %2d  %.3f kg  r %.4f  half %.4f  pos %s  rot %s", p.index, p.name,
			p.parent, p.mass, p.radius, p.half, vec(p.pos), vec(p.rot)))
	end
end)

concommand.Add("dmr_nm_joints", function(ply)
	if not allowed(ply) then return end
	if not load(ply) then return end
	local names = DMR.PartNames()
	for _, j in ipairs(DMR.BuildJoints()) do
		local limits = j.kind == "hinge" and string.format("[%.4f, %.4f]", j.limits.x, j.limits.y) or vec(j.limits)
		say(ply, string.format("  %-16s <- %-16s %-5s pivot %s  frame %s  limits %s  armature %g", names[j.part + 1],
			names[j.parent + 1], j.kind, vec(j.pivot), vec(j.frame), limits, j.armature))
	end
end)
