-- NM's body drawn over the ragdolls in the world (ROADMAP M12, docs/adr/0030), the server's side: for the superadmins
-- who turned it on (the Skeletons tab, dmr_rig_view), every fitted ragdoll with NM near them 15 times a second (each
-- part's frame from the fork's native body) and its parts' shapes once a body (cl_rig_view.lua draws them).

util.AddNetworkString("dmr_rig_view")

local watchers = {} -- player -> {[ent] = the native body whose shapes it has}
local RANGE = 3000

net.Receive("dmr_rig_view", function(_, ply)
	if not IsValid(ply) or not ply:IsSuperAdmin() then return end
	watchers[ply] = net.ReadBool() and (watchers[ply] or {}) or nil
end)

hook.Add("PlayerDisconnected", "DMR_RigView", function(ply) watchers[ply] = nil end)

local nextSend = 0
hook.Add("Think", "DMR_RigView", function()
	if not next(watchers) or CurTime() < nextSend then return end
	nextSend = CurTime() + 1 / 15
	for ply, sent in pairs(watchers) do
		if not IsValid(ply) then
			watchers[ply] = nil
		else
			for _, ent in pairs(DMR_Agents) do
				if IsValid(ent) and ent.DMR_Native and ent:GetPos():DistToSqr(ply:GetPos()) < RANGE * RANGE then
					local ok, parts = pcall(DMR.RagdollParts, ent.DMR_Native)
					if ok then
						local shapes
						if sent[ent] ~= ent.DMR_Native then
							local oks, s = pcall(DMR.RagdollShapes, ent.DMR_Fit)
							shapes = oks and util.Compress(util.TableToJSON(s)) or nil
							sent[ent] = ent.DMR_Native
						end
						-- The frames may get lost; the shapes come once, so they don't.
						net.Start("dmr_rig_view", shapes == nil)
						net.WriteEntity(ent)
						net.WriteUInt(ent.DMR_Native, 32)
						net.WriteUInt(#parts, 16)
						for i = 1, #parts do net.WriteFloat(parts[i]) end
						net.WriteBool(shapes ~= nil)
						if shapes then
							net.WriteUInt(#shapes, 32)
							net.WriteData(shapes, #shapes)
						end
						net.Send(ply)
					end
				end
			end
		end
	end
end)
