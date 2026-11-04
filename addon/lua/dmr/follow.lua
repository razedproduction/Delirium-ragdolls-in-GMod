-- The FOLLOW agents (ROADMAP M4): NM drives a ragdoll's native body (ragdolls.lua puts it on) from the module's step
-- callback (module/src/follow.rs). Lua takes it off before the ragdoll goes, tells the module Source's gravity and
-- hears of the agents in Think (docs/adr/0003): feedback, falls, sleep, NaN.

local METRES_PER_UNIT = 0.0254
local say = DMR_Say

-- docs/adr/0007: how Source's gravity is made up to NM's 9.8 m/s^2 (sv_gravity is never touched).
local gravityMode = CreateConVar("dmr_gravity_mode", "nm", FCVAR_ARCHIVE,
	"How the ragdolls' gravity is made up to NM's 9.8 m/s^2: nm (always), hybrid (only touching the world), off")

-- docs/adr/0021: how many bodies NM drives at once (NM's step costs 1-2 ms each), and on how many threads.
local maxAgents = CreateConVar("dmr_max_agents", "8", FCVAR_ARCHIVE,
	"Bodies NM drives at most; more stay ordinary ragdolls (NM's step costs 1-2 ms each)")
local threads = CreateConVar("dmr_threads", "0", FCVAR_ARCHIVE,
	"Threads NM's agents step on at once (0: the CPU's cores less one, 1: the physics' thread only)")
local function setThreads() DMR.SetThreads(math.max(threads:GetInt(), 0)) end
cvars.AddChangeCallback("dmr_threads", setThreads, "DMR_Threads")
setThreads()

-- Agent id -> entity.
DMR_Agents = DMR_Agents or {}

local function mode()
	local m = gravityMode:GetString()
	if m ~= "nm" and m ~= "hybrid" and m ~= "off" then return "nm" end
	return m
end

-- Whether NM may go on one more body (dmr_max_agents): true, or false and why.
function DMR_AgentRoom()
	local n = 0
	for _, e in pairs(DMR_Agents) do
		-- Removed this frame: its agent goes as the frame ends.
		if IsValid(e) and not e:IsMarkedForDeletion() then n = n + 1 end
	end
	if n >= maxAgents:GetInt() then return false, string.format("dmr_max_agents (%d) bodies have NM already", maxAgents:GetInt()) end
	return true
end

-- The parts' PhysObj of an agent's entity: none, its native body's are the module's (ragdolls.lua) and Lua can't
-- reach them; nil for an entity without one.
local function partsOfEnt(ent)
	if ent.DMR_Native then return {} end
end

-- The agent is gone (taken off, or came off by itself): Lua forgets it (docs/adr/0021).
local function forget(ent, id, why)
	DMR_Agents[id] = nil
	if not IsValid(ent) then return end
	ent.DMR_Agent, ent.DMR_Lod, ent.DMR_StillFor = nil, nil, nil
	if ent.DMR_Rising then DMR_Unfreeze(ent) end
	-- A ragdoll's native body goes with its agent (ragdolls.lua).
	if ent.DMR_Native then DMR_RagdollGone(ent) end
	hook.Run("DMR_Detached", ent, why)
end

-- NM off the ragdoll: an ordinary ragdoll again.
function DMR_Unfollow(ent)
	local id = ent.DMR_Agent
	if not id then return end
	-- On a map's change the module has dropped its agents already (and DMR may be gone).
	if DMR then DMR.Unfollow(id) end
	forget(ent, id, "detached")
end

-- A message line to a body's agent (`bodyBalance start=true`, as the Playroom's console takes it).
function DMR_Send(ent, line)
	if not ent.DMR_Agent then return false, "no NM on it" end
	DMR_Wake(ent)
	return pcall(DMR.FollowSend, ent.DMR_Agent, line)
end

local function partsOf(id)
	local ent = DMR_Agents[id]
	if not IsValid(ent) then return end
	return ent, partsOfEnt(ent)
end

-- By event kind; reactions.lua adds the room's (freeze, unfreeze, died, say).
DMR_FollowHandlers = DMR_FollowHandlers or {}
local handlers = DMR_FollowHandlers
for k, h in pairs({
	-- NM's doll fell asleep: so do the bodies (no drives come while it sleeps); a hit wakes both.
	sleep = function(ent, parts) for _, p in ipairs(parts) do p:Sleep() end end,
	feedback = function(ent, _, e)
		local name, slot = e.text:match("^(%S+) (%d+)$")
		hook.Run("DMR_Feedback", ent, name, tonumber(slot), e.value)
	end,
	fallen = function(ent) hook.Run("DMR_Fallen", ent) end,
	detached = function(ent, _, e)
		print("[DMR] NM came off " .. tostring(ent) .. ": " .. e.text)
		forget(ent, e.id, e.text)
	end,
	-- The game moved him (SetPos): NM's frame went with him (docs/adr/0021).
	teleported = function(ent, _, e)
		local s = DMR.FollowState(e.id)
		if s and ent.DMR_Spawn then ent.DMR_Spawn.pos = Vector(s.origin.x, s.origin.y, s.origin.z) / METRES_PER_UNIT end
		hook.Run("DMR_Teleported", ent, e.value)
	end,
	-- A parked agent's bodies moved: NM is back on them (lifecycle.lua).
	unparked = function(ent) ent.DMR_Lod, ent.DMR_StillFor = nil, nil end,
}) do handlers[k] = h end

local nextMask = 0
hook.Add("Think", "DMR_Follow", function()
	local g = physenv.GetGravity() * METRES_PER_UNIT
	DMR.SetGravity(g, mode())
	for _, e in ipairs(DMR.FollowEvents()) do
		local ent, parts = partsOf(e.id)
		local h = handlers[e.kind]
		if ent and parts and h then h(ent, parts, e) end
	end
	-- docs/adr/0007: parts the game doesn't pull get no Source gravity to make up.
	if CurTime() < nextMask then return end
	nextMask = CurTime() + 0.25
	for id, ent in pairs(DMR_Agents) do
		local parts = IsValid(ent) and partsOfEnt(ent)
		if parts then
			local mask, frozen = 0, false
			for i, p in ipairs(parts) do
				if not p:IsGravityEnabled() then mask = mask + 2 ^ (i - 1) end
				frozen = frozen or not p:IsMotionEnabled()
			end
			-- The module took NM off by itself (the bench's scenario ended): forget the agent.
			if not DMR.FollowGravityOff(id, mask) then
				forget(ent, id, "detached")
			-- docs/adr/0021: a part frozen by the player (physgun, a tool) parks NM till it's unfrozen; the get-up
			-- clip's own freeze isn't.
			elseif frozen and not ent.DMR_Rising and ent.DMR_Lod ~= "frozen" and not ent.DMR_Bench then
				DMR.FollowPark(id, true, false)
				ent.DMR_Lod = "frozen"
			elseif not frozen and ent.DMR_Lod == "frozen" then
				DMR.FollowPark(id, false)
				ent.DMR_Lod = nil
			end
		else
			-- Gone (its OnRemove unfollowed it), or its bodies can't be told apart any more: NM off.
			if IsValid(ent) then DMR_Unfollow(ent) end
			DMR_Agents[id] = nil
		end
	end
end)

local function admin(ply) return not IsValid(ply) or ply:IsSuperAdmin() end

-- dmr_status: the FOLLOW agents and what they cost.
concommand.Add("dmr_status", function(ply)
	if not admin(ply) then return end
	local st = DMR.FollowStats()
	local lines = { string.format("[DMR] %d agent(s) (dmr_max_agents %d), gravity %s (Source %.2f m/s^2, NM 9.8), NM %.3f ms a physics step on %d thread(s)",
		#DMR.FollowList(), maxAgents:GetInt(), mode(), -physenv.GetGravity().z * METRES_PER_UNIT, st.ms, st.threads) }
	-- docs/adr/0009: with tuning on, NM isn't the Playroom's.
	local tuning = GetConVar("dmr_tuning")
	lines[#lines + 1] = (tuning and tuning:GetBool()) and ("  tuning ON, profile " .. GetConVar("dmr_tuning_profile"):GetString())
		or "  tuning off (the Playroom's)"
	for _, id in ipairs(DMR.FollowList()) do
		local s = DMR.FollowState(id)
		local ent = DMR_Agents[id]
		if s then
			lines[#lines + 1] = string.format("  #%d %s: step %d, %s%s%s%s, pelvis z %.3f m, head z %.3f m, compensation %.2f, NM %.3f ms a step",
				id, tostring(ent), s.steps, s.balancing and "balancing" or "not balancing", s.fallen and ", fallen" or "",
				s.asleep and ", asleep" or (s.awake and "" or ", still"),
				s.parked and (", parked (" .. tostring(IsValid(ent) and ent.DMR_Lod or "?") .. ")") or "",
				s.pelvis.z, s.head.z, s.compensation, s.cost)
		end
	end
	say(ply, table.concat(lines, "\n"))
end)

-- dmr_follow_send <message line>: to every FOLLOW body (e.g. dmr_follow_send bodyRelax start=true).
concommand.Add("dmr_follow_send", function(ply, _, _, line)
	if not admin(ply) then return end
	for _, ent in pairs(DMR_Agents) do
		if IsValid(ent) then
			local ok, err = DMR_Send(ent, line)
			if not ok then return say(ply, "[DMR] " .. tostring(err)) end
		end
	end
end)
