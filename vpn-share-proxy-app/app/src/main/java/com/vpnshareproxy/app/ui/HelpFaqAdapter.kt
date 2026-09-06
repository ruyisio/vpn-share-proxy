package com.vpnshareproxy.app.ui

import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.TextView
import androidx.recyclerview.widget.RecyclerView
import com.vpnshareproxy.app.R

data class HelpFaqItem(
    val question: String,
    val answer: String,
)

class HelpFaqAdapter(
    private val items: List<HelpFaqItem>,
) : RecyclerView.Adapter<HelpFaqAdapter.Holder>() {

    private val expanded = BooleanArray(items.size)

    class Holder(view: View) : RecyclerView.ViewHolder(view) {
        val question: TextView = view.findViewById(R.id.tvHelpQuestion)
        val answer: TextView = view.findViewById(R.id.tvHelpAnswer)
        val chevron: TextView = view.findViewById(R.id.tvHelpChevron)
    }

    override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): Holder {
        val view = LayoutInflater.from(parent.context)
            .inflate(R.layout.item_help_faq, parent, false)
        return Holder(view)
    }

    override fun getItemCount(): Int = items.size

    override fun onBindViewHolder(holder: Holder, position: Int) {
        val item = items[position]
        val open = expanded[position]
        holder.question.text = item.question
        holder.answer.text = item.answer
        holder.answer.visibility = if (open) View.VISIBLE else View.GONE
        holder.chevron.text = if (open) "−" else "+"
        holder.itemView.setOnClickListener {
            val pos = holder.adapterPosition
            if (pos == RecyclerView.NO_POSITION) return@setOnClickListener
            expanded[pos] = !expanded[pos]
            notifyItemChanged(pos)
        }
    }
}
