-- The DMR tool (docs/adr/0023): one toolgun tool for humanoid ragdolls with NM, with modes picked in its panel.
-- Everything goes through what the Playroom has (its reactions, NM's messages as its lines send them, its buttons) and
-- the public API (dmr.Attach, DMR_React, DMR_Send); nothing of NM's is tuned here. Superadmins only unless
-- dmr_tool_everyone is 1.

TOOL.Category = "DMR"
TOOL.Name = "#tool.dmr_tool.name"
TOOL.ClientConVar = {
	mode = "react",
	reaction = "shot",
	weapon = "pistol",
	behaviour = "bodyBalance",
	target = "look",
	damage = "10",
	impulse = "40",
	health = "100",
}
TOOL.Information = { { name = "left" }, { name = "right" }, { name = "reload" } }

-- Mode: its name and what the left, right and reload do.
local MODES = {
	{ "attach", "NM on / off", "NM on the ragdoll (balancing)", "NM off the ragdoll", "The Playroom's balance button" },
	{ "react", "Reactions", "The reaction where you hit", "Kill it", "Health back (200)" },
	{ "behaviour", "NM behaviour", "Start the behaviour on the ragdoll", "Stop all of its behaviours", "The Playroom's balance button" },
	{ "target", "Target", "Aim it at a thing, a player or an NPC (it keeps on it as it moves) or a point (hold: it follows your aim)", "Pick a ragdoll (else the nearest)", "Stop it" },
	{ "inspect", "Inspector", "Pick a ragdoll: its state", "Unpick", "-" },
	{ "kick", "Kick", "Hold to charge, let go to kick the part you aim at", "-", "-" },
	{ "health", "Health", "Set its health (the slider)", "Kill it", "Health back (200)" },
}
local MODE = {}
for _, m in ipairs(MODES) do MODE[m[1]] = m end

-- Target: what it does about the point, as the Playroom's lines have it (delirium-room).
local TARGETS = {
	{ "look", "Look at it (headLook)" },
	{ "point", "Point at it (pointArm)" },
	{ "lean", "Lean towards it (leanInDirection)" },
	{ "flinch", "Flinch from it (upperBodyFlinch)" },
	{ "grab", "Grab it (grab, both hands)" },
}

local REACTIONS = {
	{ "shot", "Shot (the weapon below)" }, { "blast", "Blast" }, { "burn", "Fire" }, { "taser", "Taser" },
	{ "melee", "Blow (damage, impulse below)" }, { "nearmiss", "Near miss" }, { "car", "Car" }, { "thrown", "Thrown at" },
	{ "prop", "A thing hits it (impulse below)" },
}

local WEAPONS = { "pistol", "deagle", "smg", "ak47", "sniper", "shotgun", "rpg", "grenade", "bat", "baton" }

local M = 0.0254

if SERVER then
	local everyone = CreateConVar("dmr_tool_everyone", "0", FCVAR_ARCHIVE, "The DMR tool for every player (0: superadmins only)")
	util.AddNetworkString("dmr_tool")

	local function allowed(ply)
		if everyone:GetBool() or ply:IsSuperAdmin() then return true end
		DMR_Say(ply, "[DMR] the DMR tool is for superadmins (dmr_tool_everyone)")
		return false
	end

	-- A humanoid ragdoll NM can drive (ragdolls.lua).
	local function body(ent) return IsValid(ent) and DMR_IsMapped(ent) end

	local function nm(ply, ent)
		-- A ragdoll NM drives on a fitted body (ROADMAP M10).
		if IsValid(ent) and ent.DMR_Agent then return dmr.Get(ent) end
		DMR_Say(ply, "[DMR] aim at a ragdoll with NM on it")
	end

	local function pick(ply, ent)
		ply.DMR_ToolBody = ent
		ply:SetNW2Entity("DMR_ToolBody", ent)
	end

	-- The ragdoll picked, or the one nearest to the player.
	local function picked(ply)
		local ent = ply.DMR_ToolBody
		if IsValid(ent) and ent.DMR_Agent then return ent end
		local best, bd
		for _, e in pairs(DMR_Agents) do
			local d = IsValid(e) and e:GetPos():DistToSqr(ply:GetPos())
			if d and (not bd or d < bd) then best, bd = e, d end
		end
		return best
	end

	-- The target's lines (delirium-room's: look, pointArm, the lean away from a hit, flinch, linegrab). `vel` (units/s)
	-- is a moving target's, as the Playroom's headLook at a thrown thing has it; `again` an update of the same target.
	local function aim(ent, kind, p, vel, again)
		local c = dmr.Get(ent)
		if not c then return end
		if kind == "look" then
			c:Send("headLook", { start = true, pos = p, vel = (vel or vector_origin) * M, alwaysLook = true })
		elseif kind == "point" then
			c:Send("pointArm", { start = true, target = p })
		elseif kind == "lean" then
			local d = p - ent:WorldSpaceCenter()
			d.z = 0
			c:Send("leanInDirection", { start = true, leanAmount = 0.4, dir = d:GetNormalized() })
		elseif kind == "flinch" then
			c:Send("upperBodyFlinch", { start = true, pos = p, newHit = not again })
		elseif kind == "grab" then
			-- A line across the way from him to it, 0.6 m long.
			local to = p - ent:WorldSpaceCenter()
			to.z = 0
			local right = to:Angle():Right() * (0.3 / M)
			c:Send("grab", { start = true, linegrab = true, useLeft = true, useRight = true, dontLetGo = true,
				pos1 = p + right, pos2 = p - right })
		end
	end

	local STOP = { look = "headLook", point = "pointArm", lean = "leanInDirection", flinch = "upperBodyFlinch", grab = "grab" }

	-- Targets that move: a ragdoll kept on a thing (the point hit on it, it moves with it) or on a player's or an NPC's
	-- eyes. Sent again 10 times a second while it moves, and only then.
	local tracks = {} -- ragdoll -> {ent, lpos, kind, last}
	local function track(h, ent, kind, hit)
		tracks[h] = ent and { ent = ent, lpos = ent:WorldToLocal(hit), kind = kind } or nil
		h:SetNW2Entity("DMR_Target", ent or NULL)
	end
	local function trackedAt(t)
		if t.ent:IsPlayer() or t.ent:IsNPC() then return t.ent:EyePos() end
		return t.ent:LocalToWorld(t.lpos)
	end
	local nextTrack = 0
	hook.Add("Think", "DMR_ToolTrack", function()
		if not next(tracks) or CurTime() < nextTrack then return end
		nextTrack = CurTime() + 0.1
		for h, t in pairs(tracks) do
			if not IsValid(h) or not h.DMR_Agent or not IsValid(t.ent) then
				tracks[h] = nil
				if IsValid(h) then h:SetNW2Entity("DMR_Target", NULL) end
			else
				local p = trackedAt(t)
				if not t.last or p:DistToSqr(t.last) > 1 then
					aim(h, t.kind, p, t.ent:GetVelocity(), t.last ~= nil)
					t.last = p
				end
			end
		end
	end)

	local function inspectText(ent)
		local c = dmr.Get(ent)
		local s = c and c:GetState()
		if not s then return tostring(ent) .. ": no NM on it" end
		local lines = {
			string.format("%s  agent #%d", tostring(ent), ent.DMR_Agent),
			string.format("health %.0f%s%s%s", s.health or 0, s.dead and ", dead" or "", s.rising and ", getting up" or "",
				(tonumber(s.burning) or 0) > 0 and ", burning" or ""),
			string.format("%s%s%s%s", s.balancing and "balancing" or "not balancing", s.fallen and ", fallen" or "",
				s.asleep and ", asleep" or "", s.parked and ", parked (LOD)" or ""),
			string.format("NM step %.2f ms, %d steps, get-up %s", s.cost or 0, s.steps or 0, tostring(s.getup)),
			string.format("speed %.2f m/s, tuning %s", Vector(s.com_v.x, s.com_v.y, s.com_v.z):Length(),
				GetConVar("dmr_tuning"):GetBool() and GetConVar("dmr_tuning_profile"):GetString() or "off"),
		}
		for _, f in ipairs(ent.DMR_ToolLog or {}) do
			lines[#lines + 1] = string.format("%.1f  %s%s", f.t, f.text, f.n > 1 and ("  x" .. f.n) or "")
		end
		return table.concat(lines, "\n")
	end

	-- What the inspector shows of NM's events: the last few, a repeated one counted.
	local function log(ent, text)
		local l = ent.DMR_ToolLog or {}
		ent.DMR_ToolLog = l
		for _, f in ipairs(l) do
			if f.text == text then
				f.n, f.t = f.n + 1, CurTime()
				return
			end
		end
		table.insert(l, 1, { t = CurTime(), text = text, n = 1 })
		l[7] = nil
	end
	hook.Add("DMR_Feedback", "DMR_Tool", function(ent, name, slot) log(ent, "feedback " .. tostring(name) .. " " .. tostring(slot)) end)
	hook.Add("DMR_Reaction", "DMR_Tool", function(ent, text) log(ent, text) end)
	hook.Add("DMR_Fallen", "DMR_Tool", function(ent) log(ent, "fallen") end)

	function TOOL:Act(button, tr)
		local ply = self:GetOwner()
		if not allowed(ply) then return false end
		local mode = self:GetClientInfo("mode")
		local ent, hit, eye = tr.Entity, tr.HitPos, ply:EyePos()
		if mode == "attach" then
			if not body(ent) then return false end
			if button == 1 then
				local c, why = dmr.Attach(ent)
				if not c then DMR_Say(ply, "[DMR] " .. tostring(why)) return false end
			elseif button == 2 then
				return dmr.Detach(ent)
			else
				local c = nm(ply, ent)
				return c and c:Balance() or false
			end
		elseif mode == "react" then
			if not nm(ply, ent) then return false end
			if button == 1 then
				local kind = self:GetClientInfo("reaction")
				local rest = ""
				if kind == "shot" then rest = "weapon=" .. self:GetClientInfo("weapon") end
				if kind == "melee" then rest = string.format("damage=%.2f", self:GetClientNumber("damage")) end
				-- A car keeps the Playroom's push (dmr_react's); a blow and a thing take the slider's.
				local ok, why = DMR_ReactAt(ent, kind, rest, hit, eye, kind ~= "car" and self:GetClientNumber("impulse") or nil)
				if not ok then DMR_Say(ply, "[DMR] " .. tostring(why)) end
				return ok and true or false
			end
			return DMR_React(ent, button == 2 and "kill" or "health value=200") and true or false
		elseif mode == "behaviour" then
			local c = nm(ply, ent)
			if not c then return false end
			if button == 1 then
				local ok, why = c:Send(self:GetClientInfo("behaviour"), { start = true })
				if not ok then DMR_Say(ply, "[DMR] " .. tostring(why)) end
				return ok and true or false
			end
			return (button == 2 and c:Stop() or c:Balance()) and true or false
		elseif mode == "target" then
			if button == 2 then
				if not (body(ent) and ent.DMR_Agent) then return false end
				pick(ply, ent)
				return true
			end
			local h = picked(ply)
			if not h then DMR_Say(ply, "[DMR] no ragdoll with NM") return false end
			local kind = self:GetClientInfo("target")
			if button == 3 then
				track(h, nil)
				dmr.Get(h):Send(STOP[kind] or "headLook", { start = false })
				return true
			end
			-- A thing, a player or an NPC: it keeps on it as it moves; the world: the point (held: it follows the aim).
			if IsValid(ent) and not ent:IsWorld() and ent ~= h then
				track(h, ent, kind, hit)
				return true
			end
			track(h, nil)
			aim(h, kind, hit)
			self.following = h
			self.nextFollow = CurTime() + 0.1
		elseif mode == "inspect" then
			if button == 1 then
				if not (body(ent) and ent.DMR_Agent) then return false end
				pick(ply, ent)
			elseif button == 2 then
				pick(ply, NULL)
			else
				return false
			end
		elseif mode == "kick" then
			if button ~= 1 then return false end
			self.kickAt = CurTime()
		elseif mode == "health" then
			if not nm(ply, ent) then return false end
			local line = button == 1 and string.format("health value=%.0f", self:GetClientNumber("health"))
				or button == 2 and "kill" or "health value=200"
			return DMR_React(ent, line) and true or false
		end
		return true
	end

	function TOOL:ServerThink()
		local ply = self:GetOwner()
		local mode = self:GetClientInfo("mode")
		-- Target: while the left is held it keeps aiming at where the player does.
		if mode == "target" and IsValid(self.following) then
			if not ply:KeyDown(IN_ATTACK) then
				self.following = nil
			elseif CurTime() >= self.nextFollow then
				self.nextFollow = CurTime() + 0.1
				aim(self.following, self:GetClientInfo("target"), ply:GetEyeTrace().HitPos, nil, true)
			end
		end
		-- Kick: let go, the part aimed at gets the blow (the Playroom's melee), as strong as it was held.
		if self.kickAt and not ply:KeyDown(IN_ATTACK) then
			local held = math.min((CurTime() - self.kickAt) / 1.5, 1)
			self.kickAt = nil
			local tr = ply:GetEyeTrace()
			if mode == "kick" and body(tr.Entity) and tr.Entity.DMR_Agent then
				DMR_ReactAt(tr.Entity, "melee", string.format("damage=%.2f", self:GetClientNumber("damage") * held),
					tr.HitPos, ply:EyePos(), math.max(self:GetClientNumber("impulse") * held, 1))
			end
		end
		-- Inspector: the picked ragdoll's state, 5 times a second, to this player only.
		if mode == "inspect" and IsValid(ply.DMR_ToolBody) and CurTime() >= (self.nextInspect or 0) then
			self.nextInspect = CurTime() + 0.2
			net.Start("dmr_tool", true)
			net.WriteString(inspectText(ply.DMR_ToolBody))
			net.Send(ply)
		end
	end
end

function TOOL:LeftClick(tr) if CLIENT then return true end return self:Act(1, tr) end
function TOOL:RightClick(tr) if CLIENT then return true end return self:Act(2, tr) end
function TOOL:Reload(tr) if CLIENT then return true end return self:Act(3, tr) end

function TOOL:Think()
	if SERVER then return self:ServerThink() end
	-- The hints follow the mode.
	local m = MODE[self:GetClientInfo("mode")] or MODES[1]
	if self.hintsFor ~= m[1] then
		self.hintsFor = m[1]
		language.Add("tool.dmr_tool.desc", "DMR: " .. m[2])
		language.Add("tool.dmr_tool.left", m[3])
		language.Add("tool.dmr_tool.right", m[4])
		language.Add("tool.dmr_tool.reload", m[5])
	end
end

function TOOL:Holster()
	self.following, self.kickAt = nil, nil
end

if CLIENT then
	language.Add("tool.dmr_tool.name", "DMR")
	language.Add("tool.dmr_tool.desc", "Ragdolls with NM: reactions, behaviours, targets, inspector, kick, health")
	language.Add("tool.dmr_tool.left", "")
	language.Add("tool.dmr_tool.right", "")
	language.Add("tool.dmr_tool.reload", "")

	local inspect = ""
	net.Receive("dmr_tool", function() inspect = net.ReadString() end)

	local held
	function TOOL:DrawHUD()
		local mode = self:GetClientInfo("mode")
		local ent = LocalPlayer():GetNW2Entity("DMR_ToolBody")
		if (mode == "target" or mode == "inspect") and IsValid(ent) then
			local p = ent:WorldSpaceCenter():ToScreen()
			surface.SetDrawColor(255, 196, 70)
			surface.DrawOutlinedRect(p.x - 14, p.y - 14, 28, 28)
		end
		-- The things the ragdolls are kept on.
		if mode == "target" then
			surface.SetDrawColor(255, 196, 70, 160)
			for _, h in ipairs(ents.FindByClass("prop_ragdoll")) do
				local t = h:GetNW2Bool("DMR_Agent") and h:GetNW2Entity("DMR_Target")
				if IsValid(t) then
					local a, b = h:WorldSpaceCenter():ToScreen(), t:WorldSpaceCenter():ToScreen()
					if a.visible and b.visible then surface.DrawLine(a.x, a.y, b.x, b.y) end
				end
			end
		end
		if mode == "inspect" and IsValid(ent) and inspect ~= "" then
			local y = ScrH() * 0.3 + 30
			for line in inspect:gmatch("[^\n]+") do
				draw.SimpleTextOutlined(line, "DermaDefaultBold", 24, y, color_white, 0, 0, 1, color_black)
				y = y + 16
			end
		end
		if mode == "kick" then
			if input.IsMouseDown(MOUSE_LEFT) and not vgui.CursorVisible() then held = held or CurTime() else held = nil end
			if held then
				local k = math.min((CurTime() - held) / 1.5, 1)
				local w, x, y = 200, ScrW() / 2 - 100, ScrH() / 2 + 40
				surface.SetDrawColor(0, 0, 0, 160)
				surface.DrawRect(x, y, w, 8)
				surface.SetDrawColor(255, 196 - k * 120, 70)
				surface.DrawRect(x, y, w * k, 8)
			end
		end
	end

	local function choices(panel, label, convar, list)
		local cb = panel:ComboBox(label, convar)
		for _, c in ipairs(list) do
			if istable(c) then cb:AddChoice(c[2], c[1]) else cb:AddChoice(c, c) end
		end
		return cb
	end

	function TOOL.BuildCPanel(panel)
		panel:ClearControls()
		panel:Help("Humanoid ragdolls with NM: the Playroom's reactions and NM's behaviours on them. Superadmins only unless dmr_tool_everyone is 1.")
		local mode = GetConVar("dmr_tool_mode"):GetString()
		local cb = panel:ComboBox("Mode", "dmr_tool_mode")
		for _, m in ipairs(MODES) do cb:AddChoice(m[2], m[1], m[1] == mode) end
		cb.OnSelect = function(_, _, _, v) RunConsoleCommand("dmr_tool_mode", v) end
		local m = MODE[mode] or MODES[1]
		panel:Help(string.format("Left: %s\nRight: %s\nReload: %s", m[3], m[4], m[5]))
		if mode == "react" then
			choices(panel, "Reaction", "dmr_tool_reaction", REACTIONS)
			local cat = DMR_MenuCatalogue and DMR_MenuCatalogue()
			local w = {}
			for name in SortedPairs(cat and cat.weapons or {}) do w[#w + 1] = name end
			choices(panel, "Weapon", "dmr_tool_weapon", #w > 0 and w or WEAPONS)
			panel:NumSlider("Blow's damage", "dmr_tool_damage", 0, 100, 0)
			panel:NumSlider("Impulse (N s)", "dmr_tool_impulse", 1, 400, 0)
		elseif mode == "behaviour" then
			local list = panel:ComboBox("Behaviour", "dmr_tool_behaviour")
			local function fill(cat)
				if not IsValid(list) or not cat then return end
				list:Clear()
				local cur = GetConVar("dmr_tool_behaviour"):GetString()
				for _, n in ipairs(cat.messages or {}) do list:AddChoice(n, n, n == cur) end
			end
			fill(DMR_MenuCatalogue and DMR_MenuCatalogue())
			hook.Add("DMR_Catalogue", list, function(_, cat) fill(cat) end)
			if DMR_MenuAskCatalogue and not (DMR_MenuCatalogue and DMR_MenuCatalogue()) then DMR_MenuAskCatalogue() end
			panel:TextEntry("Or type its name", "dmr_tool_behaviour")
			panel:Help("With tuning on (the DMR window), the profile's values go with it.")
		elseif mode == "target" then
			choices(panel, "It will", "dmr_tool_target", TARGETS)
		elseif mode == "kick" then
			panel:NumSlider("Damage at full charge", "dmr_tool_damage", 0, 100, 0)
			panel:NumSlider("Impulse at full charge (N s)", "dmr_tool_impulse", 1, 400, 0)
		elseif mode == "health" then
			panel:NumSlider("Health", "dmr_tool_health", 0, 200, 0)
		end
		panel:Button("Open the DMR window", "dmr_menu")
	end
	DMR_ToolBuildPanel = TOOL.BuildCPanel

	-- The panel shows the mode's options: rebuilt whenever the mode changes (its list, the console, a bind).
	cvars.AddChangeCallback("dmr_tool_mode", function()
		timer.Simple(0, function()
			local cp = controlpanel.Get("dmr_tool")
			if IsValid(cp) then DMR_ToolBuildPanel(cp) end
		end)
	end, "DMR_Tool")
end
