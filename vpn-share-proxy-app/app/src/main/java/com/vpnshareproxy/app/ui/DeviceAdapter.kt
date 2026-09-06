package com.vpnshareproxy.app.ui

import android.content.Context
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.TextView
import androidx.core.content.ContextCompat
import androidx.recyclerview.widget.RecyclerView
import com.vpnshareproxy.app.R
import com.vpnshareproxy.app.data.ConnectedDevice

class DeviceAdapter(
    private var devices: MutableList<ConnectedDevice> = mutableListOf(),
    private var proxyMode: String = "global",
    private val onPolicyChange: (ConnectedDevice, String) -> Unit
) : RecyclerView.Adapter<DeviceAdapter.ViewHolder>() {

    class ViewHolder(view: View) : RecyclerView.ViewHolder(view) {
        val tvName: TextView = view.findViewById(R.id.tvDeviceName)
        val tvDetails: TextView = view.findViewById(R.id.tvDeviceDetails)
        val tvState: TextView = view.findViewById(R.id.tvDeviceState)
        val btnToggle: TextView = view.findViewById(R.id.btnPolicyToggle)
    }

    override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): ViewHolder {
        val view = LayoutInflater.from(parent.context).inflate(R.layout.item_device, parent, false)
        return ViewHolder(view)
    }

    override fun onBindViewHolder(holder: ViewHolder, position: Int) {
        val device = devices[position]
        val ctx = holder.itemView.context
        val mode = proxyMode.lowercase()

        holder.tvName.text = device.displayName
        val kind = shareLabel(ctx, device.iface, device.ip)
        holder.tvDetails.text = "${device.ip}  ${device.mac}  ·  $kind (${device.iface})"

        val viaVpn = !device.isBypassed
        if (viaVpn) {
            holder.tvState.text = ctx.getString(R.string.state_via_vpn)
            holder.tvState.setTextColor(ContextCompat.getColor(ctx, R.color.accent))
        } else {
            holder.tvState.text = ctx.getString(R.string.state_direct)
            holder.tvState.setTextColor(ContextCompat.getColor(ctx, R.color.warn))
        }

        when (mode) {
            "global" -> {
                // Global: everyone on VPN; no per-device override in UI
                holder.btnToggle.visibility = View.GONE
            }
            "mac_allowlist", "mac_blocklist" -> {
                holder.btnToggle.visibility = View.VISIBLE
                if (viaVpn) {
                    holder.btnToggle.text = ctx.getString(R.string.action_direct)
                    holder.btnToggle.setBackgroundResource(R.drawable.bg_button_idle)
                    holder.btnToggle.setTextColor(ContextCompat.getColor(ctx, R.color.ink))
                    holder.btnToggle.setOnClickListener {
                        onPolicyChange(device, "bypass")
                    }
                } else {
                    holder.btnToggle.text = ctx.getString(R.string.action_vpn)
                    holder.btnToggle.setBackgroundResource(R.drawable.bg_button_green)
                    holder.btnToggle.setTextColor(ContextCompat.getColor(ctx, R.color.mode_selected_fg))
                    holder.btnToggle.setOnClickListener {
                        onPolicyChange(device, "allow")
                    }
                }
            }
        }
    }

    override fun getItemCount(): Int = devices.size

    fun updateData(newDevices: List<ConnectedDevice>, mode: String) {
        devices.clear()
        devices.addAll(newDevices)
        proxyMode = mode
        notifyDataSetChanged()
    }

    /**
     * Heuristic only — Android does not expose a stable "tether kind" API.
     * Prefer iface name, then classic Android tether subnets.
     */
    private fun shareLabel(ctx: Context, iface: String, ip: String): String {
        val ifc = iface.lowercase()
        when {
            ifc == "wlan1" || ifc == "ap0" || ifc == "softap0" || ifc == "swlan0" ||
                ifc.startsWith("wlan") && ifc != "wlan0" -> return ctx.getString(R.string.tether_wifi)
            ifc == "rndis0" || ifc == "usb0" || ifc.startsWith("rndis") -> return ctx.getString(R.string.tether_usb)
            ifc == "bt-pan" || ifc.startsWith("bnep") -> return ctx.getString(R.string.tether_bt)
        }
        return when {
            ip.startsWith("192.168.43.") -> ctx.getString(R.string.tether_wifi)
            ip.startsWith("192.168.42.") -> ctx.getString(R.string.tether_usb)
            ip.startsWith("192.168.44.") -> ctx.getString(R.string.tether_bt)
            else -> ctx.getString(R.string.tether_other)
        }
    }
}
