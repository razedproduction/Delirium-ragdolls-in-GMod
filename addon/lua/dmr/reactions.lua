-- The Playroom room's reactions on the bodies NM drives (ROADMAP M6, docs/adr/0016, 0020): the game's damage, near
-- misses, throws and drops become the module's reactions (DMR.FollowReact, the Playroom's own code in
-- module/src/follow/room.rs); the Playroom's switches are convars with its defaults. A fitted body stays down instead
-- of getting up by Homer's clips. Only the translation of the game's events is ours: part, point, direction, weapon.

local M = 0.0254
local say = DMR_Say

-- The Playroom's switches (settings.txt, docs/adr/0016): name, convar default, help.
local SWITCHES = {
	{ "tasks", "0", "NM -> animation as the game's tasks end (the Playroom's game tasks)" },
	{ "look", "1", "He looks at what's thrown at him" },
	{ "avoid", "0", "He flinches from what's thrown at him" },
	{ "getup", "0", "When he lies still: 0 the game's get-up clip, 1 his muscles, 2 he stays down, 3 learned (not ported)" },
	{ "extra_reactions", "0", "The Playroom's additional reactions (dmr_extra_pick picks them)" },
	{ "extra_pick", "255", "Additional reactions, a bit each: 1 windmill, 2 curl up before getting up, 4 curl up when shot at "
		.. "lying, 8 look at who hit him, 16 tumble off a ledge, 32 smartFall when dropped, 64 dodge from above, 128 brace "
		.. "on a wall" },
	{ "fire_reaction", "1", "NM's OnFire task when he burns" },
	{ "dive_shot", "0", "Shot at again, he dives" },
	{ "dodge_gap", "3", "Seconds between his dives" },
	{ "weak", "0", "The weapons' weaker taskparams sets (agility 2)" },
	{ "god", "0", "Infinite health" },
}

local convars = {}
for _, s in ipairs(SWITCHES) do
	convars[s[1]] = CreateConVar("dmr_" .. s[1], s[2], { FCVAR_ARCHIVE, FCVAR_REPLICATED }, s[3])
end

local function pushSwitches()
	if not DMR.SetReactions then return end
	local kv = {}
	for _, s in ipairs(SWITCHES) do
		kv[#kv + 1] = s[1] .. "=" .. convars[s[1]]:GetString()
	end
	local ok, err = pcall(DMR.SetReactions, table.concat(kv, " "))
	if not ok then print("[DMR] reactions: " .. tostring(err)) end
end
for _, s in ipairs(SWITCHES) do
	cvars.AddChangeCallback("dmr_" .. s[1], pushSwitches, "DMR_Reactions")
end
pushSwitches()

local logReactions = CreateConVar("dmr_react_log", "0", FCVAR_ARCHIVE, "Print the room's reactions (the Playroom's log lines)")

-- A point (units -> m) and a direction as the module takes them.
local function at(p) return string.format("%.4f,%.4f,%.4f", p.x * M, p.y * M, p.z * M) end
local function dir(d) return string.format("%.4f,%.4f,%.4f", d.x, d.y, d.z) end

-- A reaction on a body with NM (`DMR.FollowReact`'s line). True if it went.
function DMR_React(ent, line)
	if not IsValid(ent) or not ent.DMR_Agent then return false, "no NM on it" end
	-- Parked far away or lying still (lifecycle.lua): NM back on him first.
	DMR_Wake(ent)
	local ok, res = pcall(DMR.FollowReact, ent.DMR_Agent, line)
	if not ok then return false, res end
	return res
end

-- The game's weapons as the Playroom's (its guns' taskparams sets and weaponinfo.xml's damage).
local WEAPONS = {
	weapon_pistol = "pistol", weapon_357 = "deagle", weapon_smg1 = "smg", weapon_ar2 = "ak47",
	weapon_crossbow = "sniper", weapon_shotgun = "shotgun", weapon_rpg = "rpg", weapon_frag = "grenade",
	weapon_crowbar = "bat", weapon_stunstick = "baton",
}

local function weaponOf(dmg)
	local inf, att = dmg:GetInflictor(), dmg:GetAttacker()
	local w = IsValid(inf) and inf:IsWeapon() and inf or (IsValid(att) and att.GetActiveWeapon and att:GetActiveWeapon())
	local name = IsValid(w) and WEAPONS[w:GetClass()]
	if name then return name end
	if dmg:IsDamageType(DMG_BUCKSHOT) then return "shotgun" end
	return "pistol"
end
DMR_WeaponOf = weaponOf

-- Where a blast that hurt `ent` was: Source's RadiusDamage puts its centre in the damage position.
function DMR_BlastCentre(ent, dmg, pos)
	local inf = dmg:GetInflictor()
	local centre = dmg:GetDamagePosition()
	if centre:IsZero() then centre = dmg:GetReportedPosition() end
	if centre:IsZero() and IsValid(inf) and inf ~= ent then centre = inf:GetPos() end
	if centre:IsZero() then centre = pos end
	return centre
end

local function homerMass(ent)
	local m = 0
	for i = 0, ent:GetPhysicsObjectCount() - 1 do
		local p = ent:GetPhysicsObjectNum(i)
		if IsValid(p) then m = m + p:GetMass() end
	end
	return m
end

-- One shot a tick for each Homer: a shotgun's pellets are one shot (the Playroom's `pellets`), its other parts `more`.
local pending = {}
local function flushShots()
	for ent, s in pairs(pending) do
		local line = string.format("shot at=%s dir=%s weapon=%s", at(s.pos), dir(s.dir), s.weapon)
		-- On a ragdoll: the NM part of the physics bone the bullet hit (ragdolls.lua).
		if s.part then line = line .. " part=" .. s.part end
		if #s.more > 0 then line = line .. " more=" .. table.concat(s.more, ",") end
		DMR_React(ent, line)
	end
	pending = {}
end

-- The part NM's doll has nearest to a point (its capsules), for the shotgun's other pellets: the module picks the
-- first's itself.
local function partNear(ent, pos, d)
	return ent.DMR_Native and DMR_RagdollPartAt(ent, pos, d) or 9
end

-- A bullet on a body with NM, to go with the tick's others as one shot: at `pos` along `d`.
function DMR_QueueShot(ent, pos, d, weapon)
	local s = pending[ent]
	if s then
		s.more[#s.more + 1] = partNear(ent, pos, d)
	else
		if not next(pending) then timer.Simple(0, flushShots) end
		pending[ent] = { pos = pos, dir = d:GetNormalized(), weapon = weapon, more = {}, part = ent.DMR_Native and partNear(ent, pos, d) }
	end
end

local lastBlast = {}

hook.Add("EntityTakeDamage", "DMR_Reactions", function(ent, dmg)
	if not ent.DMR_Agent then return end
	local pos = dmg:GetDamagePosition()
	if pos:IsZero() then pos = ent:WorldSpaceCenter() end
	local force = dmg:GetDamageForce()
	local att, inf = dmg:GetAttacker(), dmg:GetInflictor()
	if dmg:IsBulletDamage() or dmg:IsDamageType(DMG_BUCKSHOT) then
		local d = force:IsZero() and IsValid(att) and (pos - (att.EyePos and att:EyePos() or att:GetPos())) or force
		DMR_QueueShot(ent, pos, d, weaponOf(dmg))
		-- The bullet's push is the Playroom's applyImpulse (the module's): not the game's as well.
		dmg:SetDamageForce(vector_origin)
	elseif dmg:IsExplosionDamage() then
		-- One blast a tick (the game hurts each of his bodies).
		if lastBlast[ent] ~= engine.TickCount() then
			lastBlast[ent] = engine.TickCount()
			DMR_React(ent, "blast at=" .. at(DMR_BlastCentre(ent, dmg, pos)))
		end
		dmg:SetDamageForce(vector_origin)
	elseif dmg:IsDamageType(DMG_BURN) or dmg:IsDamageType(DMG_SLOWBURN) then
		DMR_React(ent, "burn")
	elseif dmg:IsDamageType(DMG_CLUB) or dmg:IsDamageType(DMG_SLASH) then
		local head = IsValid(att) and (att.EyePos and att:EyePos() or att:GetPos()) or pos
		DMR_React(ent, string.format("melee damage=%.2f at=%s impulse=%s head=%s", dmg:GetDamage(), at(pos), at(force), at(head)))
		if dmg:IsDamageType(DMG_SHOCK) then DMR_React(ent, "taser at=" .. at(pos)) end
		dmg:SetDamageForce(vector_origin)
	elseif dmg:IsDamageType(DMG_SHOCK) then
		DMR_React(ent, "taser at=" .. at(pos))
	end
end)

-- Contacts: a car (the Playroom's taxi: its impulse on him over 8 steps, a reaction at most once a second) or a
-- thing (`obj_hit`, once a second each). The impulse is what the contact did to his part.
local CAR_HIT = 8 / 60
local function contact(ent, data)
	local other = data.HitEntity
	if not ent.DMR_Agent or not IsValid(other) or other:IsWorld() or other == ent then return end
	local p = data.PhysObject
	if not IsValid(p) then return end
	local j = (data.OurNewVelocity - data.OurOldVelocity) * p:GetMass()
	if j:Length() * M < 1 then return end
	local now = CurTime()
	if other:IsVehicle() or other:GetClass():find("^prop_vehicle") then
		if (ent.DMR_CarNext or 0) > now then return end
		local h = ent.DMR_CarHit
		if not h then
			h = { j = Vector(), pos = data.HitPos, id = other:EntIndex(), player = IsValid(other.GetDriver and other:GetDriver()) }
			ent.DMR_CarHit = h
			timer.Simple(CAR_HIT, function()
				if not IsValid(ent) then return end
				ent.DMR_CarHit, ent.DMR_CarNext = nil, CurTime() + 1
				DMR_React(ent, string.format("car id=%d impulse=%s at=%s player=%d", h.id, at(h.j), at(h.pos), h.player and 1 or 0))
			end)
		end
		h.j:Add(j)
		return
	end
	ent.DMR_PropHit = ent.DMR_PropHit or {}
	if (ent.DMR_PropHit[other] or 0) > now then return end
	ent.DMR_PropHit[other] = now + 1
	DMR_React(ent, string.format("prop at=%s impulse=%s", at(data.HitPos), at(j)))
end

hook.Add("DMR_Detached", "DMR_Reactions", function(ent)
	if IsValid(ent) then
		ent:SetNW2Bool("DMR_Agent", false)
	end
end)

hook.Add("DMR_Attached", "DMR_Reactions", function(ent)
	if not ent.DMR_Contact then
		ent.DMR_Contact = ent:AddCallback("PhysicsCollide", contact)
	end
end)

-- A bullet that misses him (the Playroom's `near_miss`: the module tells how near).
hook.Add("EntityFireBullets", "DMR_Reactions", function(shooter, data)
	if not next(DMR_Agents) then return end
	local src, d = data.Src, data.Dir:GetNormalized()
	local tr = util.TraceLine({ start = src, endpos = src + d * (data.Distance or 56756), filter = shooter, mask = MASK_SHOT })
	for _, ent in pairs(DMR_Agents) do
		if IsValid(ent) and tr.Entity ~= ent then
			local c = ent:WorldSpaceCenter()
			local along = (c - src):Dot(d)
			-- In front of the shooter, and not stopped by a wall before it gets to him.
			if along > 0 and along <= tr.Fraction * (data.Distance or 56756) + 64 then
				DMR_React(ent, string.format("nearmiss from=%s dir=%s", at(src), dir(d)))
			end
		end
	end
end)

-- Thrown at him (physgun, gravity gun): the Playroom's `throw` (he looks, and flinches with avoid); Homer himself let
-- go of: its `fall` (smartFall with the additional reactions).
local function thrown(ply, prop)
	timer.Simple(0, function()
		if not IsValid(prop) then return end
		if prop.DMR_Agent then
			if prop:GetVelocity():Length() * M > 1 then DMR_React(prop, "dropped") end
			return
		end
		local v = prop:GetVelocity()
		if v:Length() * M < 3 then return end
		for _, ent in pairs(DMR_Agents) do
			local to = IsValid(ent) and ent:WorldSpaceCenter() - prop:GetPos()
			if to and to:Length() * M < 30 and to:GetNormalized():Dot(v:GetNormalized()) > 0.8 then
				DMR_React(ent, string.format("thrown from=%s vel=%s", at(prop:GetPos()), at(v)))
			end
		end
	end)
end
hook.Add("PhysgunDrop", "DMR_Reactions", thrown)
hook.Add("GravGunOnDropped", "DMR_Reactions", thrown)
hook.Add("GravGunPunt", "DMR_Reactions", thrown)

-- Getting up by animation is Homer's (his clips don't fit a fitted body, which stays down): nothing of a native body's
-- is Lua's to hold, so a clip only marks the agent while the module has it.
local function freeze(ent)
	ent.DMR_Rising = true
end

function DMR_Unfreeze(ent)
	ent.DMR_Rising = nil
end

local H = DMR_FollowHandlers
H.freeze = function(ent, parts, e)
	-- The clip was given up before Lua heard of it (the player took him, lifecycle.lua).
	local r = DMR.FollowRoom(e.id)
	if r and not r.rising then return end
	freeze(ent)
end
H.unfreeze = function(ent, _, e)
	DMR_Unfreeze(ent)
	DMR.FollowRiseDone(e.id)
end
H.died = function(ent) hook.Run("DMR_Died", ent) end
H.say = function(ent, _, e)
	if logReactions:GetBool() then print(string.format("[DMR] %s %.2f %s", tostring(ent), e.value, e.text)) end
	hook.Run("DMR_Reaction", ent, e.text)
end

-- The module's {x, y, z}.
local function V(t) return Vector(t.x, t.y, t.z) end

-- The HUD and the window on the clients: NM's view of each body, 20 times a second.
local nextNet = 0
hook.Add("Think", "DMR_Reactions", function()
	if CurTime() < nextNet then return end
	nextNet = CurTime() + 0.05
	for id, ent in pairs(DMR_Agents) do
		local r = IsValid(ent) and DMR.FollowRoom(id)
		local s = r and DMR.FollowState(id)
		if r and s then
			-- Burning (the Playroom's fire) while the game has him on fire.
			if ent:IsOnFire() then DMR_React(ent, "burn") end
			ent:SetNW2Bool("DMR_Agent", true)
			if ent.DMR_Spawn then ent:SetNW2Float("DMR_Floor", ent.DMR_Spawn.pos.z) end
			ent:SetNW2Float("DMR_Health", r.health)
			ent:SetNW2Bool("DMR_Dead", r.dead)
			ent:SetNW2Bool("DMR_Rising", r.rising)
			ent:SetNW2Vector("DMR_V", V(s.com_v))
		end
	end
end)

local function admin(ply) return not IsValid(ply) or ply:IsSuperAdmin() end

-- The humanoid ragdoll the player looks at (or the only one / all for the server console).
local function aimed(ply)
	if not IsValid(ply) then return nil end
	local tr = ply:GetEyeTrace()
	local ent = tr.Entity
	if IsValid(ent) and DMR_IsMapped(ent) then return ent, tr end
end

local LOOK = "look at a humanoid ragdoll"

-- dmr_attach: NM on the ragdoll looked at (balancing): any humanoid model (ragdolls.lua), as it lies.
concommand.Add("dmr_attach", function(ply)
	if not admin(ply) then return end
	local ent = aimed(ply)
	if not ent then
		local tr = IsValid(ply) and ply:GetEyeTrace()
		local why = tr and IsValid(tr.Entity) and tr.Entity:GetClass() == "prop_ragdoll" and DMR_WhyNotMapped(tr.Entity)
		return say(ply, "[DMR] dmr_attach: " .. (why or LOOK))
	end
	local function attach()
		if not IsValid(ent) then return end
		local c, why = dmr.Attach(ent)
		if not c then return say(ply, "[DMR] dmr_attach: " .. tostring(why)) end
		local fit = ent.DMR_Native and string.format(" (fitted body, %s: %.2f m, %.1f kg%s)", ent:GetNW2String("DMR_FitRig"), ent:GetNW2Float("DMR_FitHeight"),
			ent:GetNW2Float("DMR_FitMass"), ent:GetNW2String("DMR_FitWarn") ~= "" and ("; " .. ent:GetNW2String("DMR_FitWarn")) or "") or ""
		say(ply, "[DMR] NM on " .. tostring(ent) .. fit)
	end
	-- A model's first use: its body fitted in the background first (ROADMAP M12).
	DMR_WhenReady(ent:GetModel(), ply, function(_, failed) if not failed then attach() end end)
end)

-- dmr_detach: NM off the ragdoll looked at (an ordinary ragdoll, moving on as it was).
concommand.Add("dmr_detach", function(ply)
	if not admin(ply) then return end
	local ent = aimed(ply)
	if not ent then return say(ply, "[DMR] dmr_detach: " .. LOOK) end
	if not ent.DMR_Agent then return say(ply, "[DMR] dmr_detach: NM isn't on " .. tostring(ent)) end
	dmr.Detach(ent)
	say(ply, "[DMR] NM off " .. tostring(ent))
end)

-- dmr_detach_all: NM off every body.
concommand.Add("dmr_detach_all", function(ply)
	if not admin(ply) then return end
	local n = 0
	for _, ent in pairs(table.Copy(DMR_Agents)) do
		if IsValid(ent) then dmr.Detach(ent) n = n + 1 end
	end
	say(ply, string.format("[DMR] NM off %d bodies", n))
end)

-- dmr_send <message line>: to the body looked at (every body from the server console).
concommand.Add("dmr_send", function(ply, _, _, line)
	if not admin(ply) then return end
	local targets = IsValid(ply) and { (aimed(ply)) } or DMR_Agents
	for _, ent in pairs(targets) do
		local ok, err = DMR_Send(ent, line)
		if not ok then return say(ply, "[DMR] dmr_send: " .. tostring(err)) end
	end
end)

-- A reaction of the Playroom's on `ent` at a point `hit` from `from` (the game's frame, units): without at= or from= in
-- `rest`, they're added as each reaction takes them (shot, blast 1 m from it, taser, melee from `from`, ...).
-- `impulse` (N s) is the blow's, a car's or a thing's push, by default dmr_react's. True, or false and why.
function DMR_ReactAt(ent, kind, rest, hit, from, impulse)
	local line = kind .. " " .. (rest or "")
	if not line:find("at=") and not line:find("from=") then
		local d = (hit - from):GetNormalized()
		if kind == "shot" or kind == "taser" then
			line = line .. string.format(" at=%s dir=%s", at(hit), dir(d))
		elseif kind == "blast" then
			line = line .. " at=" .. at(hit - d * 40)
		elseif kind == "melee" then
			line = line .. string.format(" at=%s impulse=%s head=%s", at(hit), at(d * ((impulse or 10) / M)), at(from))
		elseif kind == "nearmiss" then
			line = line .. string.format(" from=%s dir=%s", at(from), dir((hit + Vector(0, 0, 30) - from):GetNormalized()))
		elseif kind == "thrown" then
			line = line .. string.format(" from=%s vel=%s", at(from), at(d * (8 / M)))
		elseif kind == "car" or kind == "prop" then
			line = line .. string.format(" at=%s impulse=%s", at(hit), at(d * ((impulse or 400) / M)))
		end
	end
	return DMR_React(ent, line)
end

-- dmr_react <reaction> [key=value ...]: a reaction of the Playroom's on the body looked at (every body from the
-- server console). Without at= the point looked at (shot, blast 1 m from it, taser, melee from the player).
concommand.Add("dmr_react", function(ply, _, args)
	if not admin(ply) then return end
	local kind = args[1]
	if not kind then
		return say(ply, "[DMR] dmr_react shot|prop|blast|burn|taser|melee|nearmiss|car|thrown|dropped|button|health|kill "
			.. "[key=value ...] (docs/api.md); buttons: " .. table.concat(DMR.Reactions().buttons, ", "))
	end
	local rest = table.concat(args, " ", 2)
	local ent, tr = aimed(ply)
	local targets = ent and { ent } or (IsValid(ply) and {} or DMR_Agents)
	if not next(targets) then return say(ply, "[DMR] dmr_react: look at a ragdoll with NM on it") end
	for _, e in pairs(targets) do
		local hit = tr and tr.HitPos or e:WorldSpaceCenter()
		local from = IsValid(ply) and ply:EyePos() or (hit + Vector(0, -200, 0))
		local ok, err = DMR_ReactAt(e, kind, rest, hit, from)
		if not ok then return say(ply, "[DMR] dmr_react: " .. tostring(err)) end
	end
end)
