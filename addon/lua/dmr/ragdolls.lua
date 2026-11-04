-- GMod's ragdolls (ROADMAP M10–M12, docs/adr/0025, 0027, 0030): NM on any humanoid prop_ragdoll whose skeleton is
-- mapped (ValveBiped, a known rig, a saved mapping: skeletons.lua). The model's skeleton is read from its .mdl and
-- Homer's build fitted to it (DMR.RagdollFit, kept by the model, its checksum and mapping); the fork makes
-- the fitted body natively in the ragdoll's environment (DMR.RagdollBody, VBOX3D_NM_006), posed and moving as the
-- ragdoll is (or standing in NM's stance), and NM drives it in FOLLOW as it drives Homer. The model's own prop_ragdoll
-- stays the entity the game sees (networked, traced, damaged, undone): its objects are frozen, without collisions and
-- without their own constraints, and put where their bones are on the NM body every tick (physics bones no joint is,
-- hair or coats, carried by their nearest mapped ancestor's part); the client finishes the bones NM has a part for
-- that have no object (cl_ragdolls.lua; ValveBiped's 6). Detaching gives the model its physics back, moving as the NM body was:
-- a new ragdoll of the model in its pose (its own constraints can't be put back), the old one gone.
--
-- The physgun and the gravity gun grab NM's body (the Playroom's grab, DMR.FollowGrab) instead of the frozen objects;
-- bullets hit the NM part of the physics bone they hit.

local M = 0.0254
local say = DMR_Say

local autoattach = CreateConVar("dmr_autoattach_ragdolls", "0", { FCVAR_ARCHIVE, FCVAR_REPLICATED },
	"NM on humanoid ragdolls spawned from the spawn menu (they balance where they land)")

local function V(t) return Vector(t.x, t.y, t.z) end
local function A(t) return Angle(t.x, t.y, t.z) end

local function loadData()
	if DMR.DataInfo() then return true end
	return pcall(DMR.LoadData, DMR_GameDir())
end

-- A prop_ragdoll of a humanoid model NM can drive (not Homer's own): ValveBiped, a known rig, or one with a saved
-- mapping (skeletons.lua, ROADMAP M12).
function DMR_IsMapped(ent)
	return IsValid(ent) and ent:GetClass() == "prop_ragdoll" and ent:GetPhysicsObjectCount() > 1 and DMR_ModelMapped(ent:GetModel())
end

-- An entity of any class (an NPC, a player) whose model is such a humanoid with a ragdoll.
function DMR_HasMapped(ent)
	return IsValid(ent) and util.IsValidRagdoll(ent:GetModel()) and DMR_ModelMapped(ent:GetModel())
end

-- M10–M11.5's names: any mapped humanoid now.
DMR_IsValveBiped, DMR_HasValveBiped = DMR_IsMapped, DMR_HasMapped

-- Why NM can't drive a ragdoll (for the player): not a ragdoll, not a humanoid, an unknown rig.
function DMR_WhyNotMapped(ent)
	if not IsValid(ent) or ent:GetClass() ~= "prop_ragdoll" then return "not a ragdoll" end
	if ent:GetPhysicsObjectCount() < 2 then return ent:GetModel() .. " has no ragdoll physics" end
	local c, why = DMR_ModelClass(ent:GetModel())
	if not c then return why end
	return c.why ~= "" and c.why or (ent:GetModel() .. ": can't be fitted")
end

-- Homer's build fitted to the model by its rig, or the player's (saved, or being tried: skeletons.lua), kept by the
-- module: its fit, or nil and why.
function DMR_RagdollFit(model)
	if not loadData() then return nil, "NM's data isn't loaded" end
	local bytes, why = DMR_ModelBytes(model)
	if not bytes then return nil, why end
	local ok, f = pcall(DMR.RagdollFit, model, bytes, DMR_RigArgs(DMR_ModelRig(model)))
	if not ok then return nil, tostring(f) end
	return f
end

-- A model's first NM body makes its 21 hulls in the fork (~0.9 s at once): the fit and the hulls made ahead, a part a
-- tick (DMR.RagdollWarm), with the players told on a model's first use. Model -> the next part, or true once ready.
local warm, queue, waiting = {}, {}, {}

-- Starts preparing a model (fit, hulls) in the background; `done(ok, why)` when it's ready (at once if it is).
function DMR_PrepareModel(model, done)
	if warm[model] == true then
		if done then done(true) end
		return true
	end
	if done then
		waiting[model] = waiting[model] or {}
		table.insert(waiting[model], done)
	end
	if warm[model] then return false end
	local f, why = DMR_RagdollFit(model)
	if not f then
		warm[model] = nil
		for _, cb in ipairs(waiting[model] or {}) do cb(false, why) end
		waiting[model] = nil
		return false, why
	end
	warm[model] = 0
	queue[#queue + 1] = model
	return false
end

-- Whether a model's body is ready (fitted, hulls made).
function DMR_ModelReady(model) return warm[model] == true end

hook.Add("Tick", "DMR_RagdollsWarm", function()
	local model = queue[1]
	if not model then return end
	local ok, ms = pcall(DMR.RagdollWarm, model, game.GetWorld():GetPhysicsObject(), warm[model])
	if ok and ms then
		warm[model] = warm[model] + 1
		return
	end
	warm[model] = true
	table.remove(queue, 1)
	for _, cb in ipairs(waiting[model] or {}) do cb(true) end
	waiting[model] = nil
end)

-- A mapping saved or deleted: the model is fitted again (its hulls too: they're the fitted sizes).
function DMR_ModelChanged(model)
	if warm[model] == true then warm[model] = nil end
end

-- The ragdoll's physics objects with their bones: {name, obj, pos, index}.
local function objectsOf(ent)
	local out = {}
	for i = 0, ent:GetPhysicsObjectCount() - 1 do
		local p = ent:GetPhysicsObjectNum(i)
		if IsValid(p) then
			out[#out + 1] = { name = ent:GetBoneName(ent:TranslatePhysBoneToBone(i)), obj = p, pos = p:GetPos(), index = i }
		end
	end
	return out
end

-- Where a lying or standing ragdoll faces (degrees): forward from its thighs (the fit's `map`), as NM's face takes it.
local function facing(objs, map)
	local l, r
	for _, o in ipairs(objs) do
		if o.name == map.Thigh_Left then l = o.pos end
		if o.name == map.Thigh_Right then r = o.pos end
	end
	if not l or not r then return 0 end
	local f = Vector(0, 0, 1):Cross(r - l)
	return f:Length() > 0.1 and f:Angle().y or 0
end

-- Where a humanoid ragdoll faces (degrees), as NM's face takes it.
function DMR_RagdollFacing(ent)
	local f = DMR_RagdollFit(ent:GetModel())
	return f and facing(objectsOf(ent), f.map) or 0
end

-- The model's look while NM drives it: its objects frozen, without collisions and gravity, its own joints gone (or
-- Source's separation fix pulls the posed objects back, ADR 0025).
local function hold(ent)
	ent:RemoveInternalConstraint(-1)
	for i = 0, ent:GetPhysicsObjectCount() - 1 do
		local p = ent:GetPhysicsObjectNum(i)
		if IsValid(p) then
			p:EnableMotion(false)
			p:EnableCollisions(false)
			p:EnableGravity(false)
		end
	end
end

-- NM on a humanoid ragdoll: the fitted body takes its pose and every part's velocity (a falling ragdoll keeps
-- falling), or with `stand` ({pos, yaw}: units, degrees) stands in NM's stance there. The agent's id, or nil and why.
function DMR_AttachRagdoll(ent, stand)
	if ent.DMR_Agent then return ent.DMR_Agent end
	if not DMR_IsMapped(ent) then return nil, DMR_WhyNotMapped(ent) end
	local room, full = DMR_AgentRoom()
	if not room then return nil, full end
	local model = ent:GetModel()
	local f, why = DMR_RagdollFit(model)
	if not f then return nil, why end
	DMR_MapWorld()
	-- A velocity the game set this tick (SetVelocity, a ragdoll just made) waits for the next step in the fork, where
	-- GetBodyState doesn't see it: made the body's own first, so NM's body takes it.
	if not stand then
		for i = 0, ent:GetPhysicsObjectCount() - 1 do
			local p = ent:GetPhysicsObjectNum(i)
			if IsValid(p) and p:IsMotionEnabled() then
				p:SetVelocityInstantaneous(p:GetVelocity())
				p:SetAngleVelocityInstantaneous(p:GetAngleVelocity())
			end
		end
	end
	local objs = objectsOf(ent)
	local ok, handle, ms = pcall(DMR.RagdollBody, model, ent:GetPhysicsObjectNum(0), stand or objs)
	if not ok then return nil, "NM's body: " .. tostring(handle) end
	local yaw, floor
	if stand then
		yaw, floor = stand.yaw, stand.pos
	else
		yaw = facing(objs, f.map)
		local p = ent:GetPhysicsObjectNum(0):GetPos()
		floor = DMR_FloorBelow(p, ent) or p
	end
	local ok2, id = pcall(DMR.RagdollFollow, model, handle, math.rad(yaw), floor * M)
	if not ok2 then
		DMR.RagdollRemove(handle)
		return nil, "NM: " .. tostring(id)
	end
	hold(ent)
	-- The duplicator and saves copy it without DMR's fields; the copy gets NM again (lifecycle.lua).
	ent.PreEntityCopy, ent.PostEntityCopy = DMR_PreCopy, DMR_PostCopy
	-- A ragdoll's paste doesn't carry its Lua table: the mark that it had NM goes as a modifier.
	duplicator.StoreEntityModifier(ent, "DMR_NM", { on = true })
	local bones = {}
	for _, o in ipairs(objs) do bones[o.index] = o.name end
	ent.DMR_Agent, ent.DMR_Native, ent.DMR_Fit, ent.DMR_FitBones = id, handle, model, f.bones
	ent.DMR_PhysBones, ent.DMR_Spawn, ent.DMR_Bound, ent.DMR_NativeMs = bones, { pos = floor, yaw = yaw }, true, ms
	ent.DMR_Finish = f.finish
	DMR_Agents[id] = ent
	-- The bones the client puts right (cl_ragdolls.lua), "bone>physics parent" each, and the pelvis for its camera.
	local fin = {}
	for k, b in ipairs(f.finish) do fin[k] = b[1] .. ">" .. b[2] end
	ent:SetNW2String("DMR_Finish", table.concat(fin, ";"))
	ent:SetNW2String("DMR_Pelvis", f.map.Buttocks or "")
	ent:SetNW2Bool("DMR_Ragdoll", true)
	ent:SetNW2Float("DMR_FitHeight", f.height)
	ent:SetNW2Float("DMR_FitMass", f.mass)
	ent:SetNW2String("DMR_FitRig", f.rig)
	ent:SetNW2String("DMR_FitWarn", table.concat(f.warnings, "; "))
	-- Off (and the NM body gone) before the ragdoll goes.
	ent:CallOnRemove("DMR_Unfollow", function(e) DMR_Unfollow(e) end)
	hook.Run("DMR_Attached", ent, id)
	return id
end

-- A new ragdoll of `ent`'s model as it looks, its objects where `pose` has their bones and moving so (or where
-- `ent`'s are, still), taking `ent`'s place in undo and cleanup. The new ragdoll.
local function rebuild(ent, pose)
	local new = ents.Create("prop_ragdoll")
	new:SetModel(ent:GetModel())
	new:SetPos(ent:GetPos())
	new:SetAngles(ent:GetAngles())
	new:SetSkin(ent:GetSkin())
	for i = 0, ent:GetNumBodyGroups() - 1 do new:SetBodygroup(i, ent:GetBodygroup(i)) end
	new:SetColor(ent:GetColor())
	new:SetRenderMode(ent:GetRenderMode())
	new:SetMaterial(ent:GetMaterial())
	for i = 0, #(ent:GetMaterials() or {}) - 1 do
		local m = ent:GetSubMaterial(i)
		if m ~= "" then new:SetSubMaterial(i, m) end
	end
	if ent.GetCreator and IsValid(ent:GetCreator()) then new:SetCreator(ent:GetCreator()) end
	new:Spawn()
	new:Activate()
	new:SetCollisionGroup(ent:GetCollisionGroup())
	for i = 0, new:GetPhysicsObjectCount() - 1 do
		local p = new:GetPhysicsObjectNum(i)
		local b = pose and pose[new:GetBoneName(new:TranslatePhysBoneToBone(i))]
		local old = ent:GetPhysicsObjectNum(i)
		if IsValid(p) and b then
			local origin = V(b.pos)
			p:SetPos(origin, true)
			p:SetAngles(A(b.ang))
			p:Wake()
			-- The bone's motion carried to the object's centre of mass, its exact frame (DMR.SetBodyState).
			local com = p:LocalToWorld(p:GetMassCenter())
			local w = V(b.spin)
			DMR.SetBodyState(p, com * M, b.rot, V(b.vel) + w:Cross((com - origin) * M), w)
		elseif IsValid(p) and IsValid(old) then
			p:SetPos(old:GetPos(), true)
			p:SetAngles(old:GetAngles())
			p:Wake()
		end
	end
	undo.ReplaceEntity(ent, new)
	cleanup.ReplaceEntity(ent, new)
	return new
end

-- NM off a ragdoll it drives: the model's own physics back, moving as NM's body was (a new ragdoll in its place, the
-- frozen one gone). The new ragdoll.
function DMR_DetachRagdoll(ent)
	if not ent.DMR_Native then return nil end
	local ok, pose = pcall(DMR.PoseModel, ent.DMR_Fit, ent.DMR_Native)
	ent.DMR_Restoring = true
	DMR_Unfollow(ent)
	local new = rebuild(ent, ok and pose or nil)
	ent:Remove()
	return new
end

-- The NM body of a ragdoll goes (follow.lua's forget): the module's bodies, and unless the ragdoll is going too or
-- being given back (DMR_DetachRagdoll), its own physics back where it lies.
function DMR_RagdollGone(ent)
	local handle = ent.DMR_Native
	if not handle then return end
	ent.DMR_Native = nil
	if DMR then DMR.RagdollRemove(handle) end
	ent:SetNW2Bool("DMR_Ragdoll", false)
	-- The bench's (dev.lua) stays as it is till its next run; a live body's (live.lua) gives way to its owner.
	if ent.DMR_Restoring or ent.DMR_Bench or ent.DMR_LiveOf or ent:IsMarkedForDeletion() then return end
	-- NM came off by itself (NaN, a limit): not in the middle of its callback.
	timer.Simple(0, function()
		if not IsValid(ent) or ent.DMR_Agent then return end
		rebuild(ent, nil)
		ent:Remove()
	end)
end

-- The NM part a point on the ragdoll is on: its physics bone's, or the nearest to it (a trace along `dir` through it).
function DMR_RagdollPartAt(ent, pos, dir)
	if not ent.DMR_Native then return nil end
	local d = (dir and not dir:IsZero()) and dir:GetNormalized() or Vector(0, 0, -1)
	local tr = util.TraceLine({ start = pos - d * 16, endpos = pos + d * 16, ignoreworld = true, mask = MASK_SHOT })
	if tr.Entity == ent and tr.PhysicsBone then
		local part = (ent.DMR_FitBones or {})[ent.DMR_PhysBones[tr.PhysicsBone] or ""]
		if part then return part end
	end
	return DMR.FollowNearest(ent.DMR_Agent, pos * M)
end

-- Posing (S4): the model's objects where their bones are on NM's body, every tick; the 6 bones without objects for
-- the client, in their physics parent's frame.
hook.Add("Tick", "DMR_Ragdolls", function()
	for _, ent in pairs(DMR_Agents) do
		if IsValid(ent) and ent.DMR_Native then
			local ok, pose = pcall(DMR.PoseModel, ent.DMR_Fit, ent.DMR_Native)
			if ok then
				for i, name in pairs(ent.DMR_PhysBones) do
					local b, p = pose[name], ent:GetPhysicsObjectNum(i)
					if b and IsValid(p) then
						p:SetPos(V(b.pos), true)
						p:SetAngles(A(b.ang))
					end
				end
				for k, f in ipairs(ent.DMR_Finish or {}) do
					local b, par = pose[f[1]], pose[f[2]]
					if b and par then
						local lp, la = WorldToLocal(V(b.pos), A(b.ang), V(par.pos), A(par.ang))
						ent:SetNW2Vector("DMR_FP" .. k, lp)
						ent:SetNW2Angle("DMR_FA" .. k, la)
					end
				end
			end
		end
	end
end)

-- The physgun and the gravity gun on a ragdoll NM drives grab NM's body: the Playroom's grab (a spring on the point
-- held toward where the gun holds it), not the model's frozen objects.
local grabs = {} -- player -> {ent, kind, dist}
local GRAV_REACH, GRAV_HOLD = 250, 70

local function release(ply)
	local g = grabs[ply]
	grabs[ply] = nil
	if not g or not IsValid(g.ent) or not g.ent.DMR_Agent then return end
	DMR.FollowGrab(g.ent.DMR_Agent)
	local s = DMR.FollowState(g.ent.DMR_Agent)
	if s and Vector(s.com_v.x, s.com_v.y, s.com_v.z):Length() > 1 then DMR_React(g.ent, "dropped") end
end

local function aimAt(ply, reach)
	local tr = util.TraceLine({ start = ply:EyePos(), endpos = ply:EyePos() + ply:GetAimVector() * reach, filter = ply, mask = MASK_SHOT })
	if IsValid(tr.Entity) and tr.Entity.DMR_Native then return tr end
end

local function grab(ply, tr, kind, dist)
	local ent = tr.Entity
	local part = DMR_RagdollPartAt(ent, tr.HitPos, ply:GetAimVector()) or 0
	grabs[ply] = { ent = ent, kind = kind, dist = dist }
	DMR_Wake(ent)
	DMR.FollowGrab(ent.DMR_Agent, (ply:EyePos() + ply:GetAimVector() * dist) * M, part, tr.HitPos * M)
end

hook.Add("PhysgunPickup", "DMR_Ragdolls", function(_, ent) if ent.DMR_Native then return false end end)
hook.Add("GravGunPickupAllowed", "DMR_Ragdolls", function(_, ent) if ent.DMR_Native then return false end end)
hook.Add("GravGunPunt", "DMR_Ragdolls", function(_, ent) if ent.DMR_Native then return false end end)

local function weapon(ply)
	local w = ply:GetActiveWeapon()
	return IsValid(w) and w:GetClass()
end

hook.Add("KeyPress", "DMR_Ragdolls", function(ply, key)
	local w, g = weapon(ply), grabs[ply]
	if w == "weapon_physgun" and key == IN_ATTACK and not g then
		local tr = aimAt(ply, 4096)
		if tr then grab(ply, tr, "physgun", tr.HitPos:Distance(ply:EyePos())) end
	elseif w == "weapon_physcannon" and key == IN_ATTACK2 then
		if g then return release(ply) end
		local tr = aimAt(ply, GRAV_REACH)
		if tr then grab(ply, tr, "gravgun", GRAV_HOLD) end
	elseif w == "weapon_physcannon" and key == IN_ATTACK then
		local ent = g and g.ent
		local at
		if g then
			release(ply)
			local s = IsValid(ent) and DMR.FollowState(ent.DMR_Agent)
			at = s and Vector(s.com.x, s.com.y, s.com.z) / M
		else
			local tr = aimAt(ply, GRAV_REACH)
			ent, at = tr and tr.Entity, tr and tr.HitPos
		end
		-- The punt: the Playroom's poke along the aim.
		if IsValid(ent) and at then
			local d = ply:GetAimVector()
			DMR_React(ent, string.format("poke at=%.4f,%.4f,%.4f dir=%.4f,%.4f,%.4f", at.x * M, at.y * M, at.z * M, d.x, d.y, d.z))
		end
	end
end)

hook.Add("KeyRelease", "DMR_Ragdolls", function(ply, key)
	local g = grabs[ply]
	if g and g.kind == "physgun" and key == IN_ATTACK then release(ply) end
end)

hook.Add("Think", "DMR_RagdollGrab", function()
	for ply, g in pairs(grabs) do
		local w = IsValid(ply) and weapon(ply)
		if not IsValid(g.ent) or not g.ent.DMR_Agent or w ~= (g.kind == "physgun" and "weapon_physgun" or "weapon_physcannon") then
			if IsValid(ply) then release(ply) else grabs[ply] = nil end
		else
			DMR.FollowGrab(g.ent.DMR_Agent, (ply:EyePos() + ply:GetAimVector() * g.dist) * M)
		end
	end
end)

-- `fn()` once `model` is ready for NM (at once if it is); on its first use the player is told it's being fitted in the
-- background, and `fn(nil, why)` if it can't be.
function DMR_WhenReady(model, ply, fn)
	if DMR_ModelReady(model) then return fn() end
	local started = SysTime()
	local tell = function(text)
		if IsValid(ply) then ply:ChatPrint(text) else print(text) end
	end
	tell("[DMR] " .. model .. ": first use, fitting NM's body to it in the background...")
	DMR_PrepareModel(model, function(ok, why)
		if not ok then
			tell("[DMR] " .. model .. ": " .. tostring(why))
			return fn(nil, why)
		end
		local f = DMR_RagdollFit(model)
		tell(string.format("[DMR] %s: ready in %.1f s (%s, %.2f m, %.1f kg)%s", model, SysTime() - started, f and f.rig or "?",
			f and f.height or 0, f and f.mass or 0, f and #f.warnings > 0 and (": " .. table.concat(f.warnings, "; ")) or ""))
		fn()
	end)
end

-- Ragdolls spawned from the spawn menu, with dmr_autoattach_ragdolls.
hook.Add("PlayerSpawnedRagdoll", "DMR_Ragdolls", function(ply, model, ent)
	if not autoattach:GetBool() or not DMR_IsMapped(ent) then return end
	DMR_WhenReady(ent:GetModel(), ply, function(_, failed)
		timer.Simple(0, function()
			if failed or not IsValid(ent) then return end
			local id, why = DMR_AttachRagdoll(ent)
			if id then DMR_Send(ent, "bodyBalance start=true") else DMR_Say(ply, "[DMR] NM isn't on the ragdoll: " .. tostring(why)) end
		end)
	end)
end)

-- A model's ragdoll standing in NM's stance at pos (its feet's level) facing yaw (degrees), NM on it. The entity, or
-- nil and why.
function DMR_SpawnRagdoll(model, pos, yaw)
	if not util.IsValidModel(model) then return nil, model .. ": no such model" end
	local ent = ents.Create("prop_ragdoll")
	ent:SetModel(model)
	ent:SetPos(pos)
	ent:SetAngles(Angle(0, yaw, 0))
	ent:Spawn()
	ent:Activate()
	local id, why = DMR_AttachRagdoll(ent, { pos = pos, yaw = yaw })
	if not id then
		ent:Remove()
		return nil, why
	end
	return ent
end

local function admin(ply) return not IsValid(ply) or ply:IsSuperAdmin() end

-- dmr_ragdoll [model]: a model's ragdoll in front of the player, standing, NM on it balancing (superadmins).
concommand.Add("dmr_ragdoll", function(ply, _, args)
	if not admin(ply) then return end
	local model = args[1] or "models/humans/group01/male_07.mdl"
	if not util.IsValidModel(model) then return say(ply, "[DMR] dmr_ragdoll: " .. model .. ": no such model") end
	if not DMR_ModelMapped(model) then
		local c, why = DMR_ModelClass(model)
		return say(ply, "[DMR] dmr_ragdoll: " .. tostring(c and c.why or why))
	end
	local console = not IsValid(ply)
	DMR_WhenReady(model, ply, function(_, failed)
		if failed or (not console and not IsValid(ply)) then return end
		local pos, yaw = Vector(0, 0, 0), 0
		if IsValid(ply) then
			local tr = util.TraceLine({ start = ply:EyePos(), endpos = ply:EyePos() + ply:GetAimVector() * 150, filter = ply })
			pos, yaw = DMR_FloorAt(tr.HitPos, ply), ply:EyeAngles().y + 180
		end
		local ent, why = DMR_SpawnRagdoll(model, pos, yaw)
		if not ent then return say(ply, "[DMR] dmr_ragdoll: " .. tostring(why)) end
		if IsValid(ply) then
			undo.Create("DMR ragdoll")
			undo.AddEntity(ent)
			undo.SetPlayer(ply)
			undo.Finish()
			ply:AddCleanup("ragdolls", ent)
		end
		DMR_Send(ent, "bodyBalance start=true")
		say(ply, string.format("[DMR] %s: %s, %.2f m, %.1f kg, NM agent #%d, its body made in %.1f ms%s", model, ent:GetNW2String("DMR_FitRig"),
			ent:GetNW2Float("DMR_FitHeight"), ent:GetNW2Float("DMR_FitMass"), ent.DMR_Agent, ent.DMR_NativeMs or 0,
			ent:GetNW2String("DMR_FitWarn") ~= "" and (" (" .. ent:GetNW2String("DMR_FitWarn") .. ")") or ""))
	end)
end)

-- dmr_fit <model>: the model's fit (sizes, mass, warnings), without making anything.
concommand.Add("dmr_fit", function(ply, _, args)
	if not admin(ply) then return end
	local model = args[1] or "models/humans/group01/male_07.mdl"
	local f, why = DMR_RagdollFit(model)
	if not f then return say(ply, "[DMR] dmr_fit: " .. tostring(why)) end
	say(ply, string.format("[DMR] %s: %s, %.3f m, %.1f kg (Homer %.3f m, %.1f kg), %s in %.2f ms%s", model, f.rig, f.height, f.mass, f.homer_height,
		f.homer_mass, f.cached and "cached" or "fitted", f.ms, #f.warnings > 0 and ("\n  " .. table.concat(f.warnings, "\n  ")) or ""))
end)
