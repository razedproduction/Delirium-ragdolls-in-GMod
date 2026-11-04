-- The control and telemetry channel of tools/dmr-ctl (ROADMAP M2.5): a TCP server of the module on 127.0.0.1,
-- only while dmr_dev is 1 (off by default), behind dmr_dev_token. The bench (scripts\server.ps1) turns it on from
-- data/dmr/bench.txt, which it writes for that one start.
--
-- Console commands and Lua from dmr-ctl run here, in Think; NM messages and scenarios go to the module's bench agent.

local METRES_PER_UNIT = 0.0254

local dev = CreateConVar("dmr_dev", "0", FCVAR_PROTECTED + FCVAR_DONTRECORD,
	"1: open the dmr-ctl channel on 127.0.0.1 (development only, needs dmr_dev_token)")
local port = CreateConVar("dmr_dev_port", "27099", FCVAR_PROTECTED + FCVAR_DONTRECORD, "The dmr-ctl channel's port")
local token = CreateConVar("dmr_dev_token", "", FCVAR_PROTECTED + FCVAR_DONTRECORD + FCVAR_UNLOGGED,
	"The dmr-ctl channel's token (8+ characters), set anew on each start")

-- Lua from dmr-ctl: as a chunk, or as an expression if it isn't one; what it prints and returns, tab separated.
local function runLua(code)
	local fn = CompileString(code, "dmr-ctl", false)
	if type(fn) ~= "function" then
		local expr = CompileString("return " .. code, "dmr-ctl", false)
		if type(expr) ~= "function" then return false, tostring(fn) end
		fn = expr
	end
	local out, oldPrint = {}, print
	print = function(...)
		local parts = {}
		for i = 1, select("#", ...) do parts[i] = tostring((select(i, ...))) end
		out[#out + 1] = table.concat(parts, "\t")
		oldPrint(...)
	end
	local r = { pcall(fn) }
	print = oldPrint
	if not r[1] then
		out[#out + 1] = tostring(r[2])
		return false, table.concat(out, "\n")
	end
	local results = {}
	for i = 2, table.maxn(r) do results[#results + 1] = tostring(r[i]) end
	if #results > 0 then out[#out + 1] = table.concat(results, "\t") end
	return true, table.concat(out, "\n")
end

-- A scenario on a fitted body (ROADMAP M10): the body at the Playroom room's start (`up` m higher for its `fall`),
-- facing as there, the player where the Playroom's camera starts (4 m back, looking at it); then the module runs it.
-- `r.text`: `<up> <stage> <start x y z> <model> <lead 0|1>` (m; the stage's start, where the Playroom puts its body on
-- it, ROADMAP M5).
local function benchSpawn(r)
	local w = string.Explode(" ", r.text)
	local stage = w[2] or "room"
	local start = Vector(tonumber(w[3]) or 0, tonumber(w[4]) or 0, tonumber(w[5]) or 0) / METRES_PER_UNIT
	local scene, err = DMR.BenchScene(stage)
	if not scene then error("bench scene: " .. err) end
	DMR_MapWorld()
	for _, e in ipairs(ents.FindByClass("prop_ragdoll")) do
		if e.DMR_Bench then
			DMR_Unfollow(e)
			e:Remove()
		end
	end
	-- The player out of the way of the stage's boxes (4 m back in the room, 8 m on a stage).
	local back = stage == "room" and 4 or 8
	for _, p in ipairs(player.GetAll()) do
		p:SetPos(scene.origin + Vector(0, -back / METRES_PER_UNIT, 0))
		p:SetEyeAngles(Angle(10, 90, 0))
	end
	local up = tonumber(w[1]) or 0
	-- A body fitted to a model (`fit=`, the module asks for nothing else): in LEAD on its build alone (the reference),
	-- or in FOLLOW on its native body under the model's ragdoll, standing at the start as the Playroom puts its body.
	local model, lead = w[6], w[7] == "1"
	if not model then error("no model: a scenario here runs on a fitted body (fit=<model>)") end
	do
		local f, fwhy = DMR_RagdollFit(model)
		if not f then error(fwhy) end
		local origin = scene.origin * METRES_PER_UNIT
		if lead then
			DMR.BenchFitted(model, game.GetWorld():GetPhysicsObject(), origin)
			return
		end
		local at = scene.origin + start + Vector(0, 0, up / METRES_PER_UNIT)
		local rag = ents.Create("prop_ragdoll")
		rag:SetModel(model)
		rag:SetPos(at)
		rag:Spawn()
		rag:Activate()
		rag.DMR_Bench = true
		local ok, handle = pcall(DMR.RagdollBody, model, rag:GetPhysicsObjectNum(0), { pos = at, yaw = 0 })
		if not ok then rag:Remove() error(handle) end
		rag:RemoveInternalConstraint(-1)
		for i = 0, rag:GetPhysicsObjectCount() - 1 do
			local p = rag:GetPhysicsObjectNum(i)
			p:EnableMotion(false)
			p:EnableCollisions(false)
			p:EnableGravity(false)
		end
		local bones = {}
		for i = 0, rag:GetPhysicsObjectCount() - 1 do bones[i] = rag:GetBoneName(rag:TranslatePhysBoneToBone(i)) end
		local id = DMR.BenchFitted(model, handle, origin)
		rag.DMR_Agent, rag.DMR_Native, rag.DMR_Fit, rag.DMR_PhysBones, rag.DMR_FitBones = id, handle, model, bones, f.bones
		rag:SetNW2Bool("DMR_Ragdoll", true)
		DMR_Agents[id] = rag
		rag:CallOnRemove("DMR_Unfollow", function(e) DMR_Unfollow(e) end)
	end
end

local function answer(r)
	if r.kind == "exec" then
		game.ConsoleCommand(r.text .. "\n")
		return true, "queued for the server console (its output goes to the console log; dmr-ctl exec uses RCON for it)"
	end
	return runLua(r.text)
end

local function think()
	local g = physenv.GetGravity().z * METRES_PER_UNIT
	for _, r in ipairs(DMR.DevThink(CurTime(), engine.TickCount(), FrameTime(), -g)) do
		local ok, ran, text = pcall(r.kind == "bench_spawn" and benchSpawn or answer, r)
		if r.kind == "bench_spawn" then
			-- The module answers as the scenario ends; only a failure to start is answered here.
			if not ok then DMR.DevReply(r.conn, r.id, false, "the bench's body: " .. tostring(ran)) end
		elseif ok then
			DMR.DevReply(r.conn, r.id, ran, text or "")
		else
			DMR.DevReply(r.conn, r.id, false, tostring(ran))
		end
	end
end

local function apply()
	if not dev:GetBool() then
		if DMR.DevPort() then
			DMR.DevStop()
			print("[DMR] dmr-ctl channel off")
		end
		hook.Remove("Think", "DMR_Dev")
		return
	end
	if #token:GetString() < 8 then
		print("[DMR] dmr_dev 1 needs dmr_dev_token (8+ characters): the dmr-ctl channel stays off")
		return
	end
	local ok, err = pcall(DMR.DevStart, port:GetInt(), token:GetString())
	if not ok then
		print("[DMR] dmr-ctl channel: " .. tostring(err))
		return
	end
	hook.Add("Think", "DMR_Dev", think)
	print("[DMR] dmr-ctl channel on 127.0.0.1:" .. port:GetInt())
	-- The bench agent needs NM's data.
	if not DMR.DataInfo() then
		local loaded, info = pcall(DMR.LoadData, DMR_GameDir())
		print(loaded and ("[DMR] NM's data: " .. info.dir) or ("[DMR] NM's data isn't loaded: " .. tostring(info)))
	end
end

-- The convars are checked every second (their change callbacks don't fire for every way they're set).
local applied
local function check()
	local now = dev:GetBool() and (port:GetInt() .. " " .. token:GetString()) or "off"
	if now == applied then return end
	applied = now
	apply()
end
timer.Create("DMR_Dev", 1, 0, check)

hook.Add("ShutDown", "DMR_Dev", function()
	if DMR.DevPort() then DMR.DevStop() end
end)

-- The bench's settings (scripts\server.ps1 start), once these convars exist (a +dmr_dev on the command line would
-- come too early, and GMod blocks exec from Lua): data/dmr/bench.txt's dmr_* lines, `name value` or `name "value"`.
-- The file goes as soon as it's read, so a later normal game never opens the channel.
local bench = file.Read("dmr/bench.txt", "DATA")
if bench then
	file.Delete("dmr/bench.txt")
	for line in bench:gmatch("[^\r\n]+") do
		local name, value = line:match('^%s*(dmr_[%w_]+)%s+"(.-)"%s*$')
		if not name then name, value = line:match("^%s*(dmr_[%w_]+)%s+(%S+)%s*$") end
		if name then RunConsoleCommand(name, value) end
	end
end
check()
