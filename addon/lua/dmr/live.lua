-- Live bodies (ROADMAP M11.5, docs/adr/0029): an NPC or a player with a humanoid model becomes a server ragdoll of
-- its model with NM on it (deaths.lua's DMR_RagdollFrom: posed from its bones, moving as it was) and comes back on its
-- feet where the body is. The NPC or player itself is hidden meanwhile (not drawn, not solid, not moving, an NPC not
-- thinking or targeted), keeping its health, relationships and weapons; hits on the ragdoll are its hits (its health,
-- its death), their reactions the room's (reactions.lua). Dying as a ragdoll, the ragdoll is its death ragdoll
-- (deaths.lua's DMR_DiedAsRagdoll: fatally injured, as M11's).
--
-- NPCs: a bullet that doesn't kill it (dmr_live_npc) takes it into its ragdoll, the shot the room's (`shot at= dir=
-- weapon= part=`); it's back when the body has settled (the ShotTask and the explosion task over, standing or lying
-- still) or after dmr_live_time, standing at the body's pelvis facing as it did. Players: dmr_drunk, the Playroom's
-- `drunk` button on their ragdoll, the camera on it (cl_ragdolls.lua); dmr_drunk again stands them up there.

local M = 0.0254
local say = DMR_Say

local liveNpc = CreateConVar("dmr_live_npc", "1", { FCVAR_ARCHIVE, FCVAR_REPLICATED },
	"A humanoid NPC shot (not killed) becomes its ragdoll with NM, the Playroom's reaction to the shot, then gets up where the body is")
local liveMax = CreateConVar("dmr_live_max", "4", FCVAR_ARCHIVE,
	"NPCs that are ragdolls with NM at once (within dmr_max_agents); more take their shots as the game does")
local liveTime = CreateConVar("dmr_live_time", "15", FCVAR_ARCHIVE,
	"Seconds at most an NPC stays its ragdoll before it's back on its feet")

-- A body settled: its centre of mass slower than SETTLED (m/s) for STAND_FOR s balancing, LIE_FOR s not, after at
-- least MIN_FOR s as a ragdoll. How often it's looked at (s).
local SETTLED, STAND_FOR, LIE_FOR, MIN_FOR = 0.3, 0.5, 1.5, 1
local STEP = 0.1

-- GMod's ragdolls work with this fork (VBOX3D_NM_006).
local function possible()
	local iface = DMR.InterfaceVersion()
	return iface ~= nil and (tonumber(iface:match("(%d+)$")) or 0) >= 6
end

local function bullet(dmg) return dmg:IsBulletDamage() or dmg:IsDamageType(DMG_BUCKSHOT) end

-- Owner (an NPC, a player) -> {rag, kind ("npc", "drunk"), since, still}; kept through a Lua restart, as the agents.
DMR_Lives = DMR_Lives or {}
local lives = DMR_Lives

local function count(kind)
	local n = 0
	for _, l in pairs(lives) do if l.kind == kind then n = n + 1 end end
	return n
end

-- The owner out of the way while its ragdoll stands in for it.
local function hide(ent)
	local w = ent.GetActiveWeapon and ent:GetActiveWeapon()
	ent.DMR_LiveWas = { move = ent:GetMoveType(), weapon = IsValid(w) and w or nil, notarget = ent:IsFlagSet(FL_NOTARGET) }
	ent:SetNoDraw(true)
	ent:SetNotSolid(true)
	ent:SetMoveType(MOVETYPE_NONE)
	if IsValid(w) then w:SetNoDraw(true) end
	if ent:IsNPC() then
		ent:AddFlags(FL_NOTARGET)
		ent:AddEFlags(EFL_NO_THINK_FUNCTION)
	end
end

-- The owner back, at pos facing yaw (an NPC; a player keeps their view).
local function show(ent, pos, yaw)
	local was = ent.DMR_LiveWas or {}
	ent.DMR_LiveWas = nil
	ent:SetNoDraw(false)
	ent:SetNotSolid(false)
	if was.move then ent:SetMoveType(was.move) end
	if IsValid(was.weapon) then was.weapon:SetNoDraw(false) end
	if ent:IsNPC() then
		if not was.notarget then ent:RemoveFlags(FL_NOTARGET) end
		ent:RemoveEFlags(EFL_NO_THINK_FUNCTION)
		ent:NextThink(CurTime())
	end
	if pos then ent:SetPos(pos) end
	if yaw and ent:IsNPC() then ent:SetAngles(Angle(0, yaw, 0)) end
end

local function pelvisOf(rag)
	local s = rag.DMR_Agent and DMR.FollowState(rag.DMR_Agent)
	if s then return Vector(s.pelvis.x, s.pelvis.y, s.pelvis.z) / M end
	local p = rag:GetPhysicsObjectNum(0)
	return IsValid(p) and p:GetPos() or rag:GetPos()
end

-- Where the owner stands for its ragdoll: on the floor under the pelvis, raised till its hull is free (a little).
local function standAt(ent, rag)
	local pelvis = pelvisOf(rag)
	local floor = DMR_FloorBelow(pelvis) or pelvis
	local mins, maxs
	if ent:IsPlayer() then mins, maxs = ent:GetHull() else mins, maxs = ent:GetCollisionBounds() end
	local mask = ent:IsPlayer() and MASK_PLAYERSOLID or MASK_NPCSOLID
	for k = 0, 6 do
		local p = floor + Vector(0, 0, 2 + 8 * k)
		local tr = util.TraceHull({ start = p, endpos = p, mins = mins, maxs = maxs, mask = mask, filter = { ent, rag } })
		if not tr.StartSolid then return p end
	end
	return floor
end

-- Back on its feet where the body is, the ragdoll gone.
local function finish(ent)
	lives[ent] = nil
	if not IsValid(ent) then return end
	local rag = ent.DMR_Live
	ent.DMR_Live = nil
	ent:RemoveCallOnRemove("DMR_Live")
	if ent:IsPlayer() then ent:SetNW2Entity("DMR_Live", NULL) end
	local pos, yaw
	if IsValid(rag) then
		pos, yaw = standAt(ent, rag), DMR_RagdollFacing(rag)
		rag.DMR_LiveOf = nil
		rag:Remove()
	end
	show(ent, pos, yaw)
	hook.Run("DMR_LiveEnded", ent)
end

-- The owner into its ragdoll with NM. The ragdoll, or nil and why.
local function begin(ent, kind)
	local room, full = DMR_AgentRoom()
	if not room then return nil, full end
	local rag = DMR_RagdollFrom(ent)
	if not rag then return nil, ent:GetModel() .. " has no humanoid ragdoll NM can drive (dmr_rig)" end
	local id, why = DMR_AttachRagdoll(rag)
	if not id then
		rag:Remove()
		return nil, why
	end
	rag.DMR_LiveOf, ent.DMR_Live = ent, rag
	lives[ent] = { rag = rag, kind = kind, since = CurTime(), still = 0 }
	hide(ent)
	if ent:IsPlayer() then ent:SetNW2Entity("DMR_Live", rag) end
	-- Removed (undo, cleanup, a player leaving): its ragdoll with it; the ragdoll removed: the owner back.
	ent:CallOnRemove("DMR_Live", function(e)
		local r = e.DMR_Live
		if IsValid(r) then
			r.DMR_LiveOf = nil
			r:Remove()
		end
	end)
	rag:CallOnRemove("DMR_Live", function(r)
		local owner = r.DMR_LiveOf
		if IsValid(owner) and lives[owner] then timer.Simple(0, function() if lives[owner] then finish(owner) end end) end
	end)
	hook.Run("DMR_LiveStarted", ent, rag)
	return rag
end

-- The owner died as its ragdoll: the ragdoll is its corpse (deaths.lua), or with death ragdolls off an ordinary
-- ragdoll (an NPC's) or gone (a player's: GMod's own takes its place).
local function died(owner)
	local rag = owner.DMR_Live
	if owner.DMR_LiveDied or not IsValid(rag) then return end
	owner.DMR_LiveDied = true
	lives[owner] = nil
	owner:RemoveCallOnRemove("DMR_Live")
	rag:RemoveCallOnRemove("DMR_Live")
	rag.DMR_LiveOf = nil
	if owner:IsNPC() then
		undo.ReplaceEntity(owner, rag)
		cleanup.ReplaceEntity(owner, rag)
	end
	local ok, why = DMR_DiedAsRagdoll(rag, owner)
	if ok then return end
	if why ~= "death ragdolls are off" then print("[DMR] NM isn't on " .. tostring(owner) .. "'s death ragdoll: " .. tostring(why)) end
	if owner:IsNPC() then DMR_DetachRagdoll(rag) else rag:Remove() end
end

-- The hitgroup a bullet on the ragdoll has on its owner: the hitbox on the owner's bone that the physics bone it hit
-- is (the ragdoll's own traces have none).
local function hitGroup(owner, rag, dmg)
	local pos, d = dmg:GetDamagePosition(), dmg:GetDamageForce()
	if d:IsZero() then
		local a = dmg:GetAttacker()
		d = IsValid(a) and pos - (a.EyePos and a:EyePos() or a:GetPos()) or Vector(0, 0, -1)
	end
	d = d:GetNormalized()
	local tr = util.TraceLine({ start = pos - d * 16, endpos = pos + d * 16, ignoreworld = true, mask = MASK_SHOT,
		filter = function(e) return e == rag end })
	if tr.Entity ~= rag or not tr.PhysicsBone then return HITGROUP_GENERIC end
	local b = owner:LookupBone(rag:GetBoneName(rag:TranslatePhysBoneToBone(tr.PhysicsBone)))
	local set = owner:GetHitboxSet()
	for i = 0, (b and owner:GetHitBoxCount(set) or 0) - 1 do
		if owner:GetHitBoxBone(i, set) == b then return owner:GetHitBoxHitGroup(i, set) end
	end
	return HITGROUP_GENERIC
end

-- A hit on the ragdoll is its owner's: the game's damage on it (its health, its death, who killed it), a bullet's
-- scaled by the gamemode for the hitgroup it hit (ScaleNPCDamage, ScalePlayerDamage) as one on the owner would be.
local function forward(owner, rag, dmg)
	local d = DamageInfo()
	local att = dmg:GetAttacker()
	att = IsValid(att) and att or game.GetWorld()
	local inf = dmg:GetInflictor()
	d:SetAttacker(att)
	d:SetInflictor(IsValid(inf) and inf or att)
	d:SetDamage(dmg:GetDamage())
	d:SetDamageType(dmg:GetDamageType())
	d:SetDamagePosition(dmg:GetDamagePosition())
	d:SetReportedPosition(dmg:GetReportedPosition())
	d:SetDamageForce(dmg:GetDamageForce())
	if bullet(dmg) then
		local g = hitGroup(owner, rag, dmg)
		hook.Run(owner:IsNPC() and "ScaleNPCDamage" or "ScalePlayerDamage", owner, g, d)
	end
	-- Its game ragdoll made on the server, to be taken away for this one.
	if owner:IsNPC() and d:GetDamage() >= owner:Health() then owner:SetShouldServerRagdoll(true) end
	owner.DMR_LiveForward = true
	owner:TakeDamageInfo(d)
	owner.DMR_LiveForward = nil
end

hook.Add("EntityTakeDamage", "DMR_Live", function(ent, dmg)
	local owner = ent.DMR_LiveOf
	if IsValid(owner) then
		if not owner.DMR_LiveForward then forward(owner, ent, dmg) end
		return
	end
	local rag = ent.DMR_Live
	if rag then
		if ent.DMR_LiveForward or ent.DMR_LiveDied then return end
		-- The owner is out of the way: only a bullet of the tick that took it into its ragdoll (a shotgun's other
		-- pellets) gets to it, and is the ragdoll's as well; nothing else hurts it but through the ragdoll.
		if not bullet(dmg) or not IsValid(rag) then return true end
		local h = DMR_HitOf(ent, dmg)
		DMR_QueueShot(rag, h.pos, h.dir, h.weapon)
		dmg:SetDamageForce(vector_origin)
		if ent:IsNPC() and dmg:GetDamage() >= ent:Health() then ent:SetShouldServerRagdoll(true) end
		return
	end
	-- A shot NPC that lives: into its ragdoll, the shot the room's.
	if not ent:IsNPC() or not liveNpc:GetBool() or not bullet(dmg) or dmg:GetDamage() >= ent:Health() then return end
	if count("npc") >= liveMax:GetInt() or not possible() or not DMR_HasMapped(ent) then return end
	local h = DMR_HitOf(ent, dmg)
	local made, why = begin(ent, "npc")
	if not made then
		if why then print("[DMR] " .. tostring(ent) .. " isn't a ragdoll with NM: " .. tostring(why)) end
		return
	end
	DMR_QueueShot(made, h.pos, h.dir, h.weapon)
	-- The bullet's push is the room's (its applyImpulse).
	dmg:SetDamageForce(vector_origin)
end)

-- Killed as its ragdoll: the game's own death ragdoll gives way to it.
hook.Add("CreateEntityRagdoll", "DMR_Live", function(owner, rag)
	if not IsValid(owner) or not owner.DMR_Live then return end
	if IsValid(rag) and rag ~= owner.DMR_Live then rag:Remove() end
	died(owner)
end)
hook.Add("OnNPCKilled", "DMR_Live", function(npc) if npc.DMR_Live then died(npc) end end)

hook.Add("PlayerDeath", "DMR_Live", function(ply)
	if not ply.DMR_Live then return end
	-- GMod's client ragdoll, made this tick: removed before the clients hear of it.
	local theirs = ply:GetRagdollEntity()
	if IsValid(theirs) then theirs:Remove() end
	died(ply)
end)

-- A player that died drunk (or is spawned again drunk: their ragdoll goes): themselves again as they spawn.
hook.Add("PlayerSpawn", "DMR_Live", function(ply)
	local rag = ply.DMR_Live
	if not rag then return end
	if not ply.DMR_LiveDied and IsValid(rag) then
		rag.DMR_LiveOf = nil
		rag:Remove()
	end
	lives[ply] = nil
	ply.DMR_Live, ply.DMR_LiveDied = nil, nil
	ply:RemoveCallOnRemove("DMR_Live")
	ply:SetNW2Entity("DMR_Live", NULL)
	local was = ply.DMR_LiveWas
	ply.DMR_LiveWas = nil
	ply:SetNoDraw(false)
	ply:SetNotSolid(false)
	if was and IsValid(was.weapon) then was.weapon:SetNoDraw(false) end
end)

-- A drunk player doesn't move, shoot or switch weapons themselves: their body is NM's.
hook.Add("StartCommand", "DMR_Live", function(ply, cmd)
	if lives[ply] then
		cmd:ClearMovement()
		cmd:ClearButtons()
	end
end)
hook.Add("PlayerSwitchWeapon", "DMR_Live", function(ply) if lives[ply] then return true end end)

-- Ten times a second: the owner kept with its body (where it'll stand; the server sends what's near it), an NPC back
-- once its body has settled.
local nextLook = 0
hook.Add("Think", "DMR_Live", function()
	if CurTime() < nextLook then return end
	nextLook = CurTime() + STEP
	for ent, l in pairs(lives) do
		local rag = l.rag
		if not IsValid(ent) then
			lives[ent] = nil
		elseif not IsValid(rag) or not rag.DMR_Agent then
			-- NM came off it (NaN, a limit), or it went: the owner back.
			finish(ent)
		else
			local pelvis = pelvisOf(rag)
			ent:SetPos(DMR_FloorBelow(pelvis) or pelvis)
			if l.kind == "npc" then
				local s, r = DMR.FollowState(rag.DMR_Agent), DMR.FollowRoom(rag.DMR_Agent)
				local age = CurTime() - l.since
				if not s or not r or age >= liveTime:GetFloat() then
					finish(ent)
				else
					local v = Vector(s.com_v.x, s.com_v.y, s.com_v.z):Length()
					if r.shot or r.blast or r.rising or (v > SETTLED and not s.parked) then
						l.still = 0
					else
						l.still = l.still + STEP
					end
					if age >= MIN_FOR and l.still >= (s.balancing and STAND_FOR or LIE_FOR) then finish(ent) end
				end
			end
		end
	end
end)

-- dmr_drunk: the player into their ragdoll, the Playroom's `drunk` on it; again: back on their feet there.
concommand.Add("dmr_drunk", function(ply)
	if not IsValid(ply) then return end
	if lives[ply] then
		finish(ply)
		return say(ply, "[DMR] drunk off")
	end
	if not ply:Alive() or ply:InVehicle() then return say(ply, "[DMR] dmr_drunk: not while dead or in a vehicle") end
	if not possible() then return say(ply, "[DMR] dmr_drunk: needs the fork's VBOX3D_NM_006 (" .. tostring(DMR.InterfaceVersion()) .. ")") end
	if not DMR_HasMapped(ply) then return say(ply, "[DMR] dmr_drunk: " .. ply:GetModel() .. " has no humanoid ragdoll NM can drive (dmr_rig)") end
	local rag, why = begin(ply, "drunk")
	if not rag then return say(ply, "[DMR] dmr_drunk: " .. tostring(why)) end
	DMR_React(rag, "button name=drunk")
	say(ply, "[DMR] drunk: dmr_drunk again to get up")
end)

-- The NPCs and players that are their ragdolls now: {owner, rag, kind, since} (the bench, dmr_status).
function DMR_LiveBodies()
	local out = {}
	for ent, l in pairs(lives) do
		if IsValid(ent) then out[#out + 1] = { owner = ent, rag = l.rag, kind = l.kind, since = l.since } end
	end
	return out
end
