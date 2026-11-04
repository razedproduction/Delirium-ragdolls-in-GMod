-- The agents through the game's life (ROADMAP M7, docs/adr/0021): copies (the duplicator, saves) get NM again; LOD
-- parks NM on bodies far from every player or lying still. Removal, undo, cleanup and the map's change unfollow
-- through OnRemove and ShutDown (follow.lua, dmr_init.lua); frozen parts and teleports are follow.lua's and the
-- module's.

local M = 0.0254

local lodDistance = CreateConVar("dmr_lod_distance", "3000", FCVAR_ARCHIVE,
	"Bodies farther than this from every player (units) relax, and NM comes off them till one comes near (0: never)")
local lodStill = CreateConVar("dmr_lod_still", "5", FCVAR_ARCHIVE,
	"Bodies lying still this long (s) have NM taken off till something moves them (0: never)")
-- How long a far body relaxes before NM comes off it (s); how slowly a still one's centre of mass goes (m/s).
local RELAX_FOR = 1
local STILL = 0.02

-- NM back on a body parked far away or lying still, before a reaction or a message reaches it. A frozen one stays
-- parked till it's unfrozen (follow.lua).
function DMR_Wake(ent)
	local lod = ent.DMR_Lod
	if not lod or lod == "frozen" then return end
	ent.DMR_Lod, ent.DMR_StillFor = nil, nil
	if lod ~= "relaxing" and ent.DMR_Agent then DMR.FollowPark(ent.DMR_Agent, false) end
end

-- The duplicator and saves (gm_save is duplicator.CopyEnts, then game.CleanUpMap and duplicator.Paste) merge an
-- entity's Lua table into its copy. DMR's fields are this ragdoll's (its agent, its callbacks, its native body):
-- they stay out of it, only DMR_Copy goes, and the copy gets NM again (adoptRagdoll below).
local held = setmetatable({}, { __mode = "k" })

function DMR_PreCopy(ent)
	local t = {}
	for k, v in pairs(ent:GetTable()) do
		if isstring(k) and k:sub(1, 4) == "DMR_" then t[k] = v end
	end
	for k in pairs(t) do ent[k] = nil end
	held[ent] = t
	ent.DMR_Copy = t.DMR_Agent and "attached" or "bound"
end

function DMR_PostCopy(ent)
	ent.DMR_Copy = nil
	for k, v in pairs(held[ent] or {}) do ent[k] = v end
	held[ent] = nil
end

-- Every DMR_ field of an entity that turns up with them: a copy, or a copy from a save older than this.
local function forgetFields(ent)
	local keys = {}
	for k in pairs(ent:GetTable()) do
		if isstring(k) and k:sub(1, 4) == "DMR_" then keys[#keys + 1] = k end
	end
	for _, k in ipairs(keys) do ent[k] = nil end
end

-- A copy of a humanoid ragdoll NM drove (ROADMAP M11): its objects come frozen (as NM held the original's), so its
-- own physics first, then NM again; a copy from a save older than this has the original's fields (`stale`).
local function adoptRagdoll(ent)
	local copy = ent.DMR_Copy
	local stale = ent.DMR_Agent ~= nil and DMR_Agents[ent.DMR_Agent] ~= ent
	if copy ~= "attached" and not stale then return end
	forgetFields(ent)
	for i = 0, ent:GetPhysicsObjectCount() - 1 do
		local p = ent:GetPhysicsObjectNum(i)
		if IsValid(p) then
			p:EnableMotion(true)
			p:EnableCollisions(true)
			p:EnableGravity(true)
			p:Wake()
		end
	end
	local c, why = dmr.Attach(ent)
	if not c then print("[DMR] NM isn't on a ragdoll's copy: " .. tostring(why)) end
end

-- The mark a humanoid ragdoll with NM leaves in its copies (ragdolls.lua): a ragdoll's paste doesn't carry its Lua
-- table, only its modifiers.
duplicator.RegisterEntityModifier("DMR_NM", function(_, ent) ent.DMR_Copy = "attached" end)

local function adopt(ent)
	if IsValid(ent) and DMR_IsMapped(ent) then adoptRagdoll(ent) end
end

hook.Add("OnEntityCreated", "DMR_Lifecycle", function(ent)
	-- Its model and the duplicator's fields are there by the next tick.
	if ent:GetClass() == "prop_ragdoll" then timer.Simple(0, function() adopt(ent) end) end
end)

-- The player takes a body: a parked one gets NM back (a get-up clip, if one held it, is given up).
local function taken(ply, ent)
	if not IsValid(ent) or not ent.DMR_Agent then return end
	if ent.DMR_Rising then
		DMR.FollowRiseCancel(ent.DMR_Agent)
		DMR_Unfreeze(ent)
	end
	DMR_Wake(ent)
end
hook.Add("PhysgunPickup", "DMR_Lifecycle", function(ply, ent) taken(ply, ent) end)
hook.Add("GravGunOnPickedUp", "DMR_Lifecycle", taken)

-- LOD, twice a second: far from every player he relaxes for a second and NM comes off him (back once one comes near);
-- lying still for dmr_lod_still s NM comes off him (back once something moves him: the module tells, `unparked`).
local STEP = 0.5
local nextLod = 0
hook.Add("Think", "DMR_Lifecycle", function()
	if CurTime() < nextLod then return end
	nextLod = CurTime() + STEP
	local far, still = lodDistance:GetFloat(), lodStill:GetFloat()
	local players = player.GetAll()
	for id, ent in pairs(DMR_Agents) do
		local lod = IsValid(ent) and ent.DMR_Lod
		if IsValid(ent) and not ent.DMR_Bench and lod ~= "frozen" and lod ~= "still" then
			local s, r = DMR.FollowState(id), DMR.FollowRoom(id)
			if s and r then
				local pelvis = Vector(s.pelvis.x, s.pelvis.y, s.pelvis.z) / M
				local near = far <= 0
				for _, p in ipairs(players) do
					if near then break end
					near = p:GetPos():DistToSqr(pelvis) < far * far
				end
				if not near then
					if not lod then
						dmr.Get(ent):Relax()
						ent.DMR_Lod, ent.DMR_LodAt = "relaxing", CurTime() + RELAX_FOR
					elseif lod == "relaxing" and CurTime() >= ent.DMR_LodAt then
						DMR.FollowPark(id, true, false)
						ent.DMR_Lod = "far"
					end
				elseif lod then
					DMR_Wake(ent)
				elseif still > 0 then
					-- By how far he went: lying under NM, the bodies' velocities jitter about 0.1 m/s on the spot.
					local com, last = Vector(s.com.x, s.com.y, s.com.z), ent.DMR_StillAt
					ent.DMR_StillAt = com
					if s.balancing or r.rising or not last or com:Distance(last) > STILL * STEP then
						ent.DMR_StillFor = 0
					else
						ent.DMR_StillFor = (ent.DMR_StillFor or 0) + STEP
						if ent.DMR_StillFor >= still then
							DMR.FollowPark(id, true, true)
							ent.DMR_Lod = "still"
						end
					end
				end
			end
		end
	end
end)
