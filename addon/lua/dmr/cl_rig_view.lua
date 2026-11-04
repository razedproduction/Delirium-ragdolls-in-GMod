-- NM's body drawn over the ragdolls in the world (ROADMAP M12, rig_view.lua): each fitted ragdoll with NM near the
-- player gets its parts' capsules and boxes drawn where the fork's native body has them, so a rig tried from the
-- Skeletons tab can be watched as NM drives it. dmr_rig_view 1 (superadmins; the Skeletons tab turns it on).

local view = CreateClientConVar("dmr_rig_view", "0", false, false, "NM's body (capsules) drawn over the ragdolls with NM (superadmins)")

local bodies = {} -- ent -> {native, shapes, parts, at}

local function subscribe(on)
	net.Start("dmr_rig_view")
	net.WriteBool(on)
	net.SendToServer()
end
cvars.AddChangeCallback("dmr_rig_view", function(_, _, v) subscribe(v ~= "0") end, "DMR_RigView")
hook.Add("InitPostEntity", "DMR_RigView", function() if view:GetBool() then subscribe(true) end end)

net.Receive("dmr_rig_view", function()
	local ent, native = net.ReadEntity(), net.ReadUInt(32)
	local n = net.ReadUInt(16)
	local parts = {}
	for i = 1, n do parts[i] = net.ReadFloat() end
	local b = bodies[ent]
	if not b or b.native ~= native then
		b = { native = native }
		bodies[ent] = b
	end
	if net.ReadBool() then
		local len = net.ReadUInt(32)
		b.shapes = util.JSONToTable(util.Decompress(net.ReadData(len)) or "")
	end
	b.parts, b.at = parts, CurTime()
end)

local CAPSULE, BOX = Color(90, 200, 255, 200), Color(255, 196, 70, 200)

local function drawCapsule(a, b, r, col)
	render.DrawWireframeSphere(a, r, 6, 6, col, true)
	render.DrawWireframeSphere(b, r, 6, 6, col, true)
	local d = (b - a):GetNormalized()
	local side = d:Cross(math.abs(d.z) < 0.9 and Vector(0, 0, 1) or Vector(1, 0, 0)):GetNormalized()
	local up = d:Cross(side)
	for _, o in ipairs({ side, -side, up, -up }) do render.DrawLine(a + o * r, b + o * r, col, true) end
end

hook.Add("PostDrawTranslucentRenderables", "DMR_RigView", function(_, sky)
	if sky or not view:GetBool() then return end
	render.SetColorMaterial()
	for ent, b in pairs(bodies) do
		if not IsValid(ent) or CurTime() - b.at > 1 then
			bodies[ent] = nil
		elseif b.shapes then
			for k, s in ipairs(b.shapes) do
				local i = (k - 1) * 6
				local pos = Vector(b.parts[i + 1], b.parts[i + 2], b.parts[i + 3])
				local ang = Angle(b.parts[i + 4], b.parts[i + 5], b.parts[i + 6])
				if s.shape == "capsule" then
					local a = LocalToWorld(Vector(s.a.x, s.a.y, s.a.z), angle_zero, pos, ang)
					local e = LocalToWorld(Vector(s.b.x, s.b.y, s.b.z), angle_zero, pos, ang)
					drawCapsule(a, e, s.radius, CAPSULE)
				else
					local _, wa = LocalToWorld(vector_origin, Angle(s.ang.x, s.ang.y, s.ang.z), pos, ang)
					local h = Vector(s.half.x, s.half.y, s.half.z)
					render.DrawWireframeBox(pos, wa, -h, h, BOX, true)
				end
			end
		end
	end
end)
