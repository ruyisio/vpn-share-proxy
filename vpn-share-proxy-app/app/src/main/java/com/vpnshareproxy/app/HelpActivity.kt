package com.vpnshareproxy.app

import android.os.Bundle
import android.widget.TextView
import androidx.appcompat.app.AppCompatActivity
import androidx.recyclerview.widget.LinearLayoutManager
import androidx.recyclerview.widget.RecyclerView
import com.vpnshareproxy.app.ui.HelpFaqAdapter
import com.vpnshareproxy.app.ui.HelpFaqItem

class HelpActivity : AppCompatActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_help)

        findViewById<TextView>(R.id.btnHelpBack).setOnClickListener { finish() }

        val faq = listOf(
            HelpFaqItem(
                getString(R.string.help_q_vpn_tun),
                getString(R.string.help_a_vpn_tun),
            ),
            HelpFaqItem(
                getString(R.string.help_q_ipv4_only),
                getString(R.string.help_a_ipv4_only),
            ),
            HelpFaqItem(
                getString(R.string.help_q_ipv6),
                getString(R.string.help_a_ipv6),
            ),
            HelpFaqItem(
                getString(R.string.help_q_offload),
                getString(R.string.help_a_offload),
            ),
            HelpFaqItem(
                getString(R.string.help_q_modes),
                getString(R.string.help_a_modes),
            ),
            HelpFaqItem(
                getString(R.string.help_q_why_v6_fail),
                getString(R.string.help_a_why_v6_fail),
            ),
        )

        findViewById<RecyclerView>(R.id.rvHelpFaq).apply {
            layoutManager = LinearLayoutManager(this@HelpActivity)
            adapter = HelpFaqAdapter(faq)
        }
    }
}
