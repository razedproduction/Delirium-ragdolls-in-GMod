-- Skeletons beyond ValveBiped (ROADMAP M12, docs/adr/0030): which bone of a model is each of NM's joints, and the
-- player's edits of the fit. The module knows the common rigs by their bones' names (ValveBiped, Biped, Mixamo, Unreal,
-- TF2, VRoid, MMD) and guesses the rest by the names' words and the hierarchy (DMR.ModelClassify). A model's rig made
-- by hand (the rig editor, tools/dms-rig; or written by hand) is data/dmr/skeletons/<model>.json, text that travels to
-- another machine as it is; it's read again as the file changes, and the model's ragdolls with NM get NM again by it.
-- Models are read where the game has them (mounted Workshop GMAs included): nothing of them is copied or written.
--
-- A rig: {bones = {[part] = bone}, joints = {[part] = {x, y, z}} (units, the model's space), parts = {[part] = {radius,
-- half, box = {x, y, z} (m), mass (kg), offset = {x, y, z} (m, the part's frame), turn = {p, y, r} (degrees)}}}.

local DIR = "dmr/skeletons"
local FORMAT = 2

file.CreateDir(DIR)

-- A model's rig file (data/dmr/skeletons/<model without models/ and .mdl>.json).
local function path(model)
	local name = model:lower():gsub("\\", "/"):gsub("^models/", ""):gsub("%.mdl$", ""):gsub("[^%w_%-/%.]", "_")
	return DIR .. "/" .. name .. ".json"
end
DMR_SkeletonPath = path

local saved = {} -- model -> its rig, or false
local stamps = {} -- model -> its file's time when read (0: none)
local classes = {} -- model -> its class (DMR.ModelClassify), or {error}

-- A model's .mdl as the game has it (its own files, a mounted GMA's): bytes, or nil and why.
function DMR_ModelBytes(model)
	if not isstring(model) or not model:lower():match("%.mdl$") then return nil, tostring(model) .. ": not a model" end
	local bytes = file.Read(model, "GAME")
	if not bytes then return nil, model .. ": not found (not mounted?)" end
	return bytes
end

local function vec(v)
	if not istable(v) then return nil end
	local x, y, z = tonumber(v[1] or v.x), tonumber(v[2] or v.y), tonumber(v[3] or v.z)
	if not (x and y and z) or x ~= x or y ~= y or z ~= z then return nil end
	return { x, y, z }
end

local function zero(v) return not v or (v[1] == 0 and v[2] == 0 and v[3] == 0) end

-- A rig as a file has it made safe: names as strings, numbers as numbers, nothing that doesn't change the fit.
local function clean(t)
	local rig = { bones = {}, joints = {}, parts = {} }
	for part, bone in pairs(istable(t.bones) and t.bones or {}) do
		if isstring(part) and isstring(bone) and bone ~= "" then rig.bones[part] = bone end
	end
	for part, v in pairs(istable(t.joints) and t.joints or {}) do
		local j = vec(v)
		if isstring(part) and not zero(j) then rig.joints[part] = j end
	end
	for part, p in pairs(istable(t.parts) and t.parts or {}) do
		if isstring(part) and istable(p) then
			local e = {
				radius = tonumber(p.radius), half = tonumber(p.half), box = vec(p.box), mass = tonumber(p.mass),
				offset = vec(p.offset), turn = vec(p.turn),
			}
			if zero(e.offset) then e.offset = nil end
			if zero(e.turn) then e.turn = nil end
			if next(e) then rig.parts[part] = e end
		end
	end
	return rig
end

-- A rig from its file's text (format 1 had bones only): the rig and the model it was made for, or nil and why.
function DMR_SkeletonDecode(text)
	local t = isstring(text) and util.JSONToTable(text)
	if not istable(t) or not istable(t.bones) then return nil, "not a DMR rig (no bones)" end
	if (tonumber(t.dms_skeleton) or 0) > FORMAT then return nil, "a rig from a newer DMR (format " .. tostring(t.dms_skeleton) .. ")" end
	return clean(t), t.model
end

-- The rig of a model from data/dmr/skeletons, or nil (the model's own).
function DMR_ModelRig(model)
	if saved[model] == nil then
		local p = path(model)
		stamps[model] = file.Time(p, "DATA") or 0
		local text = file.Read(p, "DATA")
		local rig, why = nil, nil
		if text then rig, why = DMR_SkeletonDecode(text) end
		if text and not rig then print("[DMR] data/" .. p .. ": " .. tostring(why)) end
		saved[model] = rig or false
	end
	return saved[model] or nil
end

-- The module's fit arguments of a rig: its bones (nil for the model's own) and its edits (nil for none).
function DMR_RigArgs(rig)
	if not rig then return nil, nil end
	local edited = next(rig.joints) ~= nil or next(rig.parts) ~= nil
	return next(rig.bones) and rig.bones or nil, edited and rig or nil
end

local function forget(model)
	saved[model], classes[model] = nil, nil
	if DMR then DMR.RagdollForget(model) end
	if DMR_ModelChanged then DMR_ModelChanged(model) end
end

-- What a model's skeleton is (DMR.ModelClassify with its rig's bones): the class, or nil and why. Kept till its rig
-- changes.
function DMR_ModelClass(model)
	local c = classes[model]
	if not c then
		local bytes, why = DMR_ModelBytes(model)
		if bytes then
			local ok, r = pcall(DMR.ModelClassify, bytes, (DMR_RigArgs(DMR_ModelRig(model))))
			c = ok and r or { error = tostring(r) }
		else
			c = { error = why }
		end
		classes[model] = c
	end
	if c.error then return nil, c.error end
	return c
end

-- Whether NM can drive the model: a humanoid whose joints are all known (by its rig, or its file's).
function DMR_ModelMapped(model)
	-- The golden project's Homer (Delirium-physics-in-GMod, installed beside this one) is its own addon's to drive.
	if not isstring(model) or model == "" or model == "models/dms/homer.mdl" then return false end
	local c = DMR_ModelClass(model)
	return c ~= nil and c.complete
end

-- NM again on a model's ragdolls with NM, by the rig it has now: each given its own physics and NM anew, balancing
-- (death and live bodies keep theirs). How many.
local function refit(model)
	local n = 0
	for _, ent in pairs(table.Copy(DMR_Agents)) do
		if IsValid(ent) and ent.DMR_Native and ent:GetModel() == model and not ent.DMR_Death and not ent.DMR_LiveOf then
			local new = DMR_DetachRagdoll(ent)
			if IsValid(new) and DMR_AttachRagdoll(new) then
				DMR_Send(new, "bodyBalance start=true")
				n = n + 1
			end
		end
	end
	return n
end

-- The rigs' files watched (a second apart): one written or removed (by the rig editor, by hand) is read again, and the
-- model's ragdolls with NM get it at once; the superadmins are told.
timer.Create("DMR_Skeletons", 1, 0, function()
	local models = {}
	for model in pairs(stamps) do models[model] = true end
	for _, ent in pairs(DMR_Agents or {}) do
		if IsValid(ent) and ent.DMR_Native then models[ent:GetModel()] = true end
	end
	for model in pairs(models) do
		local now = file.Time(path(model), "DATA") or 0
		if stamps[model] == nil then
			stamps[model] = now
		elseif now ~= stamps[model] then
			forget(model)
			local rig = DMR_ModelRig(model)
			local c, why = DMR_ModelClass(model)
			local text = string.format("[DMR] %s: rig %s (%s)", model, rig and "reloaded from data/" .. path(model) or "file removed: the model's own",
				c and (c.complete and c.rig or c.why) or tostring(why))
			print(text)
			for _, ply in ipairs(player.GetAll()) do if ply:IsSuperAdmin() then ply:ChatPrint(text) end end
			if c and c.complete then
				DMR_WhenReady(model, nil, function(_, failed) if not failed then refit(model) end end)
			end
		end
	end
end)

-- dmr_rig <model>: what the model's skeleton is to DMR (its rig, the joints found, what's missing).
concommand.Add("dmr_rig", function(ply, _, args)
	if IsValid(ply) and not ply:IsSuperAdmin() then return end
	local model = args[1] or (IsValid(ply) and ply:GetModel()) or ""
	classes[model] = nil
	local c, why = DMR_ModelClass(model)
	if not c then return DMR_Say(ply, "[DMR] dmr_rig: " .. tostring(why)) end
	local rig = DMR_ModelRig(model)
	local lines = { string.format("[DMR] %s: %s, %s%s", model, c.rig, c.complete and "NM can drive it" or "can't be fitted",
		rig and (" (its rig: data/" .. path(model) .. ")") or "") }
	for _, part in ipairs(c.parts) do
		lines[#lines + 1] = string.format("  %-16s %s", part, c.map[part] or (c.optional[part] and "-" or "MISSING"))
	end
	for _, p in ipairs(c.problems) do lines[#lines + 1] = "  problem: " .. p end
	for _, r in ipairs(c.report) do lines[#lines + 1] = "  reader: " .. r end
	if not c.complete then lines[#lines + 1] = "  " .. c.why end
	DMR_Say(ply, table.concat(lines, "\n"))
end)
