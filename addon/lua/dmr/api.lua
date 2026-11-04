-- The public Lua API (ROADMAP M6, docs/api.md): `dmr.Attach(rag)` puts NM on a humanoid ragdoll and gives a controller
-- whose methods send NM's messages or the Playroom's reactions; hooks DMR_Attached, DMR_Detached, DMR_Fallen,
-- DMR_Feedback, DMR_Died, DMR_Reaction tell what happened (from the module's event queue, docs/adr/0003).

dmr = dmr or {}

local M = 0.0254

local Controller = {}
Controller.__index = Controller

local function at(p) return string.format("%.4f,%.4f,%.4f", p.x * M, p.y * M, p.z * M) end

-- A ragdoll's controller, or nil if NM isn't on it.
function dmr.Get(rag)
	if not IsValid(rag) or not rag.DMR_Agent then return nil end
	return setmetatable({ ent = rag }, Controller)
end

-- NM on any humanoid prop_ragdoll (ROADMAP M10: a body fitted to its model takes its pose and motion). opts:
-- `balance` (default true: bodyBalance, as the Playroom's
-- "balance" button). The controller, or nil and why.
function dmr.Attach(rag, opts)
	opts = opts or {}
	if not IsValid(rag) then return nil, "not a ragdoll" end
	local id, why
	if DMR_IsMapped(rag) then
		id, why = DMR_AttachRagdoll(rag)
	else
		return nil, DMR_WhyNotMapped(rag)
	end
	if not id then return nil, why end
	local c = dmr.Get(rag)
	if opts.balance ~= false then c:Balance() end
	return c
end

-- NM off: an ordinary ragdoll again. A fitted one gets its own physics back as a new ragdoll in the same pose and
-- motion (its own constraints were taken off for NM): true and the new entity.
function dmr.Detach(rag)
	if not IsValid(rag) or not rag.DMR_Agent then return false end
	if rag.DMR_Native then return true, DMR_DetachRagdoll(rag) end
	DMR_Unfollow(rag)
	return true
end

-- Every body with NM on it.
function dmr.All()
	local out = {}
	for _, ent in pairs(DMR_Agents) do
		if IsValid(ent) then out[#out + 1] = dmr.Get(ent) end
	end
	return out
end

-- NM's messages (`DMR.Messages`) and a message's params (`DMR.MessageParams`), for introspection.
function dmr.Messages() return DMR.Messages() end
function dmr.MessageParams(name) return DMR.MessageParams(name) end

-- The Playroom's reactions, buttons, additional reactions, get-up modes and weapons (`DMR.Reactions`).
function dmr.Reactions() return DMR.Reactions() end

function Controller:IsValid() return IsValid(self.ent) and self.ent.DMR_Agent ~= nil end
function Controller:Entity() return self.ent end
function Controller:Id() return self.ent.DMR_Agent end

-- A message line as the Playroom's console takes it: `ctrl:Send("bodyBalance", {start = true})` or
-- `ctrl:Send("bodyBalance start=true")`. Vectors (Vector) are points in the game's frame (moved into NM's) unless
-- the param is a direction; give a "x,y,z" string to pass NM's own frame through.
local DIRECTIONS = { dir = true, normal = true, bulletVel = true, impulse = true, vel = true, fallOverEdge = true }
function Controller:Send(msg, params)
	if not self:IsValid() then return false, "no NM on it" end
	local line = msg
	if params then
		local origin = self:Origin()
		local kv = {}
		for k, v in SortedPairs(params) do
			if isvector(v) then
				local p = DIRECTIONS[k] and v or (v * M - origin)
				v = string.format("%.4f,%.4f,%.4f", p.x, p.y, p.z)
			elseif isbool(v) then
				v = v and "true" or "false"
			end
			kv[#kv + 1] = k .. "=" .. tostring(v)
		end
		if #kv > 0 then line = line .. " " .. table.concat(kv, " ") end
	end
	return DMR_Send(self.ent, line)
end

-- Where NM's frame starts (m, the game's frame): its messages' points are from there. It moves with him when the
-- game teleports him (docs/adr/0021).
function Controller:Origin()
	local s = DMR.FollowState(self.ent.DMR_Agent)
	if s then return Vector(s.origin.x, s.origin.y, s.origin.z) end
	local spawn = self.ent.DMR_Spawn
	return spawn and spawn.pos * M or Vector()
end

-- A reaction of the Playroom's (`DMR.FollowReact`'s line, points in the game's frame, m; dmr_react).
function Controller:React(line) return DMR_React(self.ent, line) end

-- A bullet: `where` a part (0-20) or a point (Vector), `dir` the bullet's way (Vector), `weapon` the Playroom's
-- (pistol, deagle, smg, ak47, sniper, shotgun, ...).
function Controller:Shot(where, dir, weapon)
	local line = "shot weapon=" .. (weapon or "pistol")
	if isnumber(where) then line = line .. " part=" .. where end
	if isvector(where) then
		dir = dir or (where - self.ent:WorldSpaceCenter()):GetNormalized() * -1
		line = line .. string.format(" at=%s dir=%.4f,%.4f,%.4f", at(where), dir.x, dir.y, dir.z)
	end
	return self:React(line)
end

-- The Playroom's buttons.
function Controller:Button(name) return self:React("button name=" .. name:gsub(" ", "_")) end
function Controller:Balance() return self:Button("balance") end
function Controller:CatchFall() return self:Button("catchfall") end
function Controller:Relax() return self:Button("relax") end
function Controller:Stop() return self:Button("STOP all") end

-- A flinch: from a point (Vector) as the game's flinch task (upperBodyFlinch), or the Playroom's button.
function Controller:Flinch(from)
	if isvector(from) then return self:Send("upperBodyFlinch", { start = true, pos = from }) end
	return self:Button("flinch")
end

function Controller:Kill() return self:React("kill") end
function Controller:SetHealth(h) return self:React("health value=" .. h) end

-- NM's state: `{steps, awake, asleep, balancing, fallen, pelvis, head, com, com_v (m), health, dead, rising, burning,
-- shock, getup}`.
function Controller:GetState()
	local id = self.ent.DMR_Agent
	local s = id and DMR.FollowState(id)
	if not s then return nil end
	local r = DMR.FollowRoom(id) or {}
	for _, k in ipairs({ "health", "dead", "rising", "burning", "shock", "getup" }) do s[k] = r[k] end
	return s
end

-- A message param's default and doc (`DMR.MessageParams`): `{name, kind, doc, default, used, min, max}`.
function Controller:GetParam(msg, param)
	for _, p in ipairs(DMR.MessageParams(msg) or {}) do
		if p.name == param then return p end
	end
end
