-- The player's tuning of NM's behaviours (docs/adr/0009, ROADMAP M6), the server's side: profiles in
-- data/dmr/profiles/<name>.json hold only what differs from the Playroom's defaults (messages' params and taskparams'
-- values); "Playroom" is the empty one and can't be changed. dmr_tuning 0|1 turns it off/on, dmr_tuning_profile picks
-- the profile. Only superadmins change anything; the menu (cl_menu.lua) talks to this through net messages, and
-- changes act from Homers' next NM step. The bench and golden runs never get any (the module skips scenarios).

local DIR = "dmr/profiles"
local PLAYROOM = "Playroom"

local tuningOn = CreateConVar("dmr_tuning", "0", { FCVAR_ARCHIVE, FCVAR_REPLICATED }, "The player's tuning of NM's behaviours (0 = the Playroom's, docs/adr/0009)")
local profileName = CreateConVar("dmr_tuning_profile", PLAYROOM, { FCVAR_ARCHIVE, FCVAR_REPLICATED }, "The tuning profile (data/dmr/profiles/<name>.json)")

util.AddNetworkString("dmr_tuning")

local function clean(name)
	name = tostring(name or ""):gsub("[^%w_%- ]", ""):sub(1, 48)
	return name ~= "" and name or nil
end

local function path(name) return DIR .. "/" .. name:lower():gsub(" ", "_") .. ".json" end

-- A profile: `{messages = {[msg] = {[param] = "value"}}, taskparams = {["task set param"] = number}}`.
function DMR_TuningLoad(name)
	if name == PLAYROOM then return { messages = {}, taskparams = {} } end
	local t = util.JSONToTable(file.Read(path(name), "DATA") or "")
	if not t then return nil end
	t.messages, t.taskparams = t.messages or {}, t.taskparams or {}
	return t
end

function DMR_TuningList()
	local out = { PLAYROOM }
	for _, f in ipairs(file.Find(DIR .. "/*.json", "DATA")) do
		local t = util.JSONToTable(file.Read(DIR .. "/" .. f, "DATA") or "")
		if t and t.name and t.name ~= PLAYROOM then out[#out + 1] = t.name end
	end
	return out
end

local function save(name, t)
	file.CreateDir(DIR)
	t.name = name
	file.Write(path(name), util.TableToJSON(t, true))
end

-- The active profile to the module (`DMR.SetTuning`): its lines, or off.
function DMR_TuningApply()
	if not DMR.SetTuning or not DMR.DataInfo() then return end
	local name = profileName:GetString()
	local t = tuningOn:GetBool() and DMR_TuningLoad(name)
	if not t then
		pcall(DMR.SetTuning)
		return
	end
	local lines = {}
	for msg, ps in SortedPairs(t.messages) do
		for p, v in SortedPairs(ps) do lines[#lines + 1] = string.format("msg %s %s %s", msg, p, tostring(v)) end
	end
	for key, v in SortedPairs(t.taskparams) do lines[#lines + 1] = string.format("tp %s %s", key, tostring(v)) end
	local ok, err = pcall(DMR.SetTuning, name, table.concat(lines, "\n"))
	if not ok then print("[DMR] tuning " .. name .. ": " .. tostring(err)) end
end
cvars.AddChangeCallback("dmr_tuning", DMR_TuningApply, "DMR_Tuning")
cvars.AddChangeCallback("dmr_tuning_profile", DMR_TuningApply, "DMR_Tuning")
hook.Add("DMR_Attached", "DMR_Tuning", DMR_TuningApply)

-- The server's convars the menu sets (`dmr_<name>`): the Playroom's switches (reactions.lua), the agents' limits
-- (follow.lua, lifecycle.lua), NM on death ragdolls (deaths.lua) and live NPCs (live.lua). Nothing else: dmr_game_dir
-- and dmr_dev aren't the menu's.
local SWITCHES = {
	"tasks", "look", "avoid", "getup", "extra_reactions", "extra_pick", "fire_reaction", "dive_shot", "dodge_gap",
	"weak", "god", "max_agents", "threads", "lod_distance", "lod_still", "react_log",
	"autoattach_npc", "autoattach_players", "autoattach_max", "autoattach_time", "live_npc", "live_max", "live_time",
}
local isSwitch = {}
for _, s in ipairs(SWITCHES) do isSwitch[s] = true end

-- Their values and help texts (the clients don't have the server's convars).
local function switches()
	local out = {}
	for _, s in ipairs(SWITCHES) do
		local cv = GetConVar("dmr_" .. s)
		if cv then out[s] = { value = cv:GetString(), help = cv:GetHelpText() } end
	end
	return out
end

-- What the Debug tab shows: the versions that have to match, NM's data and what keeps DMR from working.
local function about()
	local d = DMR.DataInfo()
	return {
		module = DMR.Version,
		addon = DMR_ADDON_VERSION,
		delirium = DMR.DeliriumVersion,
		interface = DMR.InterfaceVersion() or "none",
		gameDir = DMR_GameDir(),
		data = d and string.format("%s (%s), NM's build (Homer's, the fit's template) %d parts", d.dir, d.platform, d.parts) or "not loaded",
		problems = DMR_Problems(),
	}
end

-- What the menu needs: NM's behaviours and their params, the Playroom's reaction table, the profiles.
local function catalogue()
	-- The menu may open before any Homer: NM's data from the game as a spawn loads it (taskparams, weapons need it).
	local why
	if not DMR.DataInfo() then
		local ok, err = pcall(DMR.LoadData, DMR_GameDir())
		if not ok then why = "NM's data isn't loaded: " .. tostring(err) end
	end
	-- Only the names: a behaviour's params come when the menu picks it (all of them at once overflow the channel).
	return {
		messages = DMR.Messages() or {},
		reactions = DMR.Reactions(),
		weapons = DMR.Weapons(),
		taskparams = not why and DMR.TaskParams() or {},
		profiles = DMR_TuningList(),
		profile = profileName:GetString(),
		on = tuningOn:GetBool(),
		switches = switches(),
		about = about(),
		error = why,
	}
end

-- Replies go in pieces, one a tick, so a big one (the taskparams) can't overflow the client's reliable channel.
local CHUNK = 16000
local outbox, nextReply = {}, 0
local function reply(ply, kind, t)
	local s = util.Compress(util.TableToJSON({ kind = kind, data = t }))
	nextReply = nextReply % 65535 + 1
	local n = math.max(math.ceil(#s / CHUNK), 1)
	for i = 1, n do
		outbox[#outbox + 1] = { ply = ply, id = nextReply, i = i, n = n, data = s:sub((i - 1) * CHUNK + 1, i * CHUNK) }
	end
end

hook.Add("Think", "DMR_Tuning", function()
	local m = table.remove(outbox, 1)
	if not m or not IsValid(m.ply) then return end
	net.Start("dmr_tuning")
	net.WriteUInt(m.id, 16)
	net.WriteUInt(m.i, 16)
	net.WriteUInt(m.n, 16)
	net.WriteUInt(#m.data, 32)
	net.WriteData(m.data, #m.data)
	net.Send(m.ply)
end)

-- The menu's requests: {kind, ...}. Superadmins only.
net.Receive("dmr_tuning", function(_, ply)
	if not IsValid(ply) or not ply:IsSuperAdmin() then return end
	local n = net.ReadUInt(32)
	local req = util.JSONToTable(util.Decompress(net.ReadData(n)) or "")
	if not req then return end
	local k = req.kind
	if k == "catalogue" then
		return reply(ply, "catalogue", catalogue())
	elseif k == "params" then
		local ok, ps = pcall(DMR.MessageParams, tostring(req.name))
		return reply(ply, "params", { name = req.name, params = ok and ps or {} })
	elseif k == "profile" then
		local name = clean(req.name) or PLAYROOM
		return reply(ply, "profile", { name = name, profile = DMR_TuningLoad(name) or { messages = {}, taskparams = {} } })
	elseif k == "save" then
		local name = clean(req.name)
		if not name or name == PLAYROOM then return reply(ply, "error", "the Playroom profile can't be changed: save as another name") end
		save(name, { messages = req.profile and req.profile.messages or {}, taskparams = req.profile and req.profile.taskparams or {} })
		-- SetString, not RunConsoleCommand: that one acts next frame, after the catalogue below has gone out with the
		-- old profile, and the menu then loaded the old one over what had just been saved.
		profileName:SetString(name)
	elseif k == "delete" then
		local name = clean(req.name)
		if not name or name == PLAYROOM then return end
		file.Delete(path(name))
		if profileName:GetString() == name then profileName:SetString(PLAYROOM) end
	elseif k == "use" then
		profileName:SetString(clean(req.name) or PLAYROOM)
		tuningOn:SetBool(req.on and true or false)
	elseif k == "reset" then
		-- Back to the Playroom: tuning off, the Playroom's profile.
		tuningOn:SetBool(false)
		profileName:SetString(PLAYROOM)
	elseif k == "react" then
		-- A reaction or a message on the Homer the menu picked (live try-out).
		local ent = Entity(tonumber(req.ent) or -1)
		if IsValid(ent) and ent.DMR_Agent then
			if req.line then DMR_Send(ent, req.line) else DMR_React(ent, req.react or "") end
		end
		return
	elseif k == "switch" then
		local cv = isSwitch[req.name] and GetConVar("dmr_" .. req.name)
		local v = tonumber(req.value)
		if not cv or not v then return end
		cv:SetString(tostring(v))
		return reply(ply, "switches", switches())
	end
	-- The convars' callbacks miss a profile saved again under the same name.
	DMR_TuningApply()
	timer.Simple(0, function() if IsValid(ply) then reply(ply, "catalogue", catalogue()) end end)
end)

-- dmr_tuning_status: the active profile's values.
concommand.Add("dmr_tuning_status", function(ply)
	if IsValid(ply) and not ply:IsSuperAdmin() then return end
	local name = profileName:GetString()
	local t = DMR_TuningLoad(name)
	local lines = { string.format("[DMR] tuning %s, profile %s%s", tuningOn:GetBool() and "on" or "off (the Playroom's)", name, t and "" or " (missing)") }
	for msg, ps in SortedPairs(t and t.messages or {}) do
		for p, v in SortedPairs(ps) do lines[#lines + 1] = string.format("  %s %s = %s", msg, p, tostring(v)) end
	end
	for key, v in SortedPairs(t and t.taskparams or {}) do lines[#lines + 1] = string.format("  taskparams %s = %s", key, tostring(v)) end
	DMR_Say(ply, table.concat(lines, "\n"))
end)
