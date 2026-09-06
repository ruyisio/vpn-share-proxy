package com.vpnshareproxy.app.ui

import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.TextView
import androidx.core.content.ContextCompat
import androidx.recyclerview.widget.RecyclerView
import com.vpnshareproxy.app.R
import com.vpnshareproxy.app.data.NetIface

class InterfaceAdapter(
    private var items: MutableList<NetIface> = mutableListOf(),
    private val onToggle: (NetIface) -> Unit
) : RecyclerView.Adapter<InterfaceAdapter.VH>() {

    class VH(view: View) : RecyclerView.ViewHolder(view) {
        val name: TextView = view.findViewById(R.id.tvIfaceName)
        val badge: TextView = view.findViewById(R.id.tvIfaceBadge)
        val meta: TextView = view.findViewById(R.id.tvIfaceMeta)
        val managed: TextView = view.findViewById(R.id.tvIfaceManaged)
    }

    override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): VH {
        val v = LayoutInflater.from(parent.context).inflate(R.layout.item_iface, parent, false)
        return VH(v)
    }

    override fun onBindViewHolder(holder: VH, position: Int) {
        val item = items[position]
        val ctx = holder.itemView.context
        holder.name.text = item.name
        holder.badge.visibility = if (item.recommended) View.VISIBLE else View.GONE
        holder.meta.text = buildString {
            append(item.operstate)
            if (item.hint.isNotBlank()) {
                append(" · ")
                append(item.hint)
            }
        }
        if (item.managed) {
            holder.managed.text = ctx.getString(R.string.iface_selected)
            holder.managed.setBackgroundResource(R.drawable.bg_mode_selected)
            holder.managed.setTextColor(ContextCompat.getColor(ctx, R.color.mode_selected_fg))
        } else {
            holder.managed.text = ctx.getString(R.string.iface_unselected)
            holder.managed.setBackgroundResource(R.drawable.bg_chip_idle)
            holder.managed.setTextColor(ContextCompat.getColor(ctx, R.color.mode_idle_fg))
        }
        holder.itemView.setOnClickListener { onToggle(item) }
    }

    override fun getItemCount(): Int = items.size

    fun update(data: List<NetIface>) {
        items.clear()
        items.addAll(data)
        notifyDataSetChanged()
    }
}
