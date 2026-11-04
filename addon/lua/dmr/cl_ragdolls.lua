-- GMod's ragdolls with NM on them (ROADMAP M10–M12, ragdolls.lua), the clients' part: the server poses the model's
-- physics objects from NM's body, and the bones NM has a part for but the model has no object for (ValveBiped's Spine,
-- Spine1, Spine4, Neck1 and clavicles; whatever the rig has) are put right here as it's drawn: each in its physics
-- parent's frame as the server sends it (DMR_FP<k>, DMR_FA<k>; which bones in DMR_Finish), the bones under it that
-- have no object of their own carried along. A client needs nothing but the model itself (ROADMAP M12).

-- The model's bones: which are finished (and from which parent, DMR_Finish's "bone>parent;..."), which have an object.
local function layout(ent)
	local fin, phys = {}, {}
	local k = 0
	for pair in ent:GetNW2String("DMR_Finish"):gmatch("[^;]+") do
		k = k + 1
		local bone, parent = pair:match("^(.-)>(.*)$")
		local b, p = bone and ent:LookupBone(bone), parent and ent:LookupBone(parent)
		if b and p then fin[b] = { k = k, parent = p } end
	end
	for i = 0, ent:GetBoneCount() - 1 do
		local pb = ent:TranslateBoneToPhysBone(i)
		phys[i] = pb and pb >= 0 and ent:TranslatePhysBoneToBone(pb) == i or nil
	end
	return { fin = fin, phys = phys }
end

local function finish(ent, count)
	if not ent:GetNW2Bool("DMR_Ragdoll") then return end
	local l = ent.DMR_Layout
	local key = ent:GetNW2String("DMR_Finish")
	if not l or l.key ~= key then
		l = layout(ent)
		l.key = key
		ent.DMR_Layout = l
	end
	local delta = {}
	for i = 0, count - 1 do
		local f = l.fin[i]
		if f then
			local pm, old = ent:GetBoneMatrix(f.parent), ent:GetBoneMatrix(i)
			if pm and old then
				local m = Matrix()
				m:SetTranslation(ent:GetNW2Vector("DMR_FP" .. f.k))
				m:SetAngles(ent:GetNW2Angle("DMR_FA" .. f.k))
				local new = pm * m
				ent:SetBoneMatrix(i, new)
				delta[i] = new * old:GetInverseTR()
			end
		elseif not l.phys[i] then
			local p = ent:GetBoneParent(i)
			local d = p and p >= 0 and delta[p]
			local old = d and ent:GetBoneMatrix(i)
			if old then
				ent:SetBoneMatrix(i, d * old)
				delta[i] = d
			end
		end
	end
end

-- A server ragdoll's bones on the client (C_ServerRagdoll) are built without the BuildBonePositions callback, so
-- they're finished as it's drawn: set up, put right, then drawn from the bones as they are now (a model already
-- overridden is drawn by its own override after).
local function render(ent, flags)
	ent:SetupBones()
	finish(ent, ent:GetBoneCount())
	if ent.DMR_OldRender then return ent.DMR_OldRender(ent, flags) end
	ent:DrawModel(flags)
end

-- A player's death ragdoll (deaths.lua) in the player's colour: the PlayerColor material proxy asks the entity.
local function playerColor(ent) return ent:GetNW2Vector("DMR_PlayerColor") end

local nextLook = 0
hook.Add("Think", "DMR_Ragdolls", function()
	if CurTime() < nextLook then return end
	nextLook = CurTime() + 0.25
	for _, ent in ipairs(ents.FindByClass("prop_ragdoll")) do
		if not ent.GetPlayerColor and IsValid(ent:GetNW2Entity("DMR_Owner")) then ent.GetPlayerColor = playerColor end
		local on = ent:GetNW2Bool("DMR_Ragdoll")
		if on and not ent.DMR_Finishing then
			ent.DMR_Finishing, ent.DMR_OldRender = true, ent.RenderOverride
			ent.RenderOverride = render
		elseif not on and ent.DMR_Finishing then
			ent.DMR_Finishing, ent.RenderOverride, ent.DMR_OldRender = nil, ent.DMR_OldRender, nil
		end
	end
end)

-- The death camera on a player's server death ragdoll, as GMod's (HL2MP's) follows its client one: over the
-- ragdoll's root, CHASE units back along the view, stopped by walls; the mouse turns it.
local CHASE, DEAD_VIEW, WALL = 96, 14, Vector(6, 6, 6)
local function chase(ply, rag, angles, fov)
	local pelvis = rag:GetNW2String("DMR_Pelvis")
	local b = rag:LookupBone(pelvis ~= "" and pelvis or "ValveBiped.Bip01_Pelvis")
	local root = b and rag:GetBonePosition(b) or rag:GetPos()
	local eye = root + Vector(0, 0, DEAD_VIEW)
	local tr = util.TraceHull({ start = eye, endpos = eye - angles:Forward() * CHASE, mins = -WALL, maxs = WALL, mask = MASK_SOLID, filter = { ply, rag } })
	return { origin = tr.HitPos, angles = angles, fov = fov, drawviewer = false }
end

hook.Add("CalcView", "DMR_DeathCamera", function(ply, _, angles, fov)
	-- Drunk (live.lua): the same camera on the player's ragdoll while they live.
	local live = ply:GetNW2Entity("DMR_Live")
	if ply:Alive() and IsValid(live) then return chase(ply, live, angles, fov) end
	if ply:Alive() then return end
	local mode = ply:GetObserverMode()
	if mode ~= OBS_MODE_NONE and mode ~= OBS_MODE_DEATHCAM and mode ~= OBS_MODE_FREEZECAM then return end
	local rag = ply:GetNW2Entity("DMR_Corpse")
	if not IsValid(rag) then return end
	return chase(ply, rag, angles, fov)
end)

-- Drunk: no weapon in the hands or crosshair, and the player's own moves and buttons aren't predicted (the server
-- ignores them, live.lua).
local function drunk()
	local ply = LocalPlayer()
	return IsValid(ply) and IsValid(ply:GetNW2Entity("DMR_Live"))
end
hook.Add("PreDrawViewModel", "DMR_Live", function() if drunk() then return true end end)
hook.Add("HUDShouldDraw", "DMR_Live", function(name) if name == "CHudCrosshair" and drunk() then return false end end)
hook.Add("StartCommand", "DMR_Live", function(ply, cmd)
	if ply == LocalPlayer() and drunk() then
		cmd:ClearMovement()
		cmd:ClearButtons()
	end
end)
