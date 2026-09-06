package com.vpnshareproxy.app.ui

import android.content.Context
import android.view.LayoutInflater
import android.view.View
import android.widget.LinearLayout
import android.widget.TextView
import androidx.core.os.LocaleListCompat
import com.google.android.material.bottomsheet.BottomSheetDialog
import com.vpnshareproxy.app.R

data class SheetOption(
    val id: String,
    val icon: String,
    val title: String,
    val subtitle: String? = null,
    val detail: String? = null,
    val isSelected: Boolean = false
)

data class DnsPreset(
    val id: String,
    val nameRes: Int,
    val descRes: Int,
    val icon: String,
    val ipv4: String,
    val ipv6: String
)

object SelectionBottomSheet {

    val DNS_PRESETS = listOf(
        DnsPreset(
            id = "ali",
            nameRes = R.string.dns_preset_ali,
            descRes = R.string.dns_preset_ali_desc,
            icon = "⚡",
            ipv4 = "223.5.5.5",
            ipv6 = "2400:3200::1"
        ),
        DnsPreset(
            id = "tencent",
            nameRes = R.string.dns_preset_tencent,
            descRes = R.string.dns_preset_tencent_desc,
            icon = "🐧",
            ipv4 = "119.29.29.29",
            ipv6 = "2402:4e00::"
        ),
        DnsPreset(
            id = "cloudflare",
            nameRes = R.string.dns_preset_cf,
            descRes = R.string.dns_preset_cf_desc,
            icon = "⚡",
            ipv4 = "1.1.1.1",
            ipv6 = "2606:4700:4700::1111"
        ),
        DnsPreset(
            id = "google",
            nameRes = R.string.dns_preset_google,
            descRes = R.string.dns_preset_google_desc,
            icon = "🌐",
            ipv4 = "8.8.8.8",
            ipv6 = "2001:4860:4860::8888"
        ),
        DnsPreset(
            id = "114",
            nameRes = R.string.dns_preset_114,
            descRes = R.string.dns_preset_114_desc,
            icon = "🇨🇳",
            ipv4 = "114.114.114.114",
            ipv6 = "240C::6666"
        ),
        DnsPreset(
            id = "quad9",
            nameRes = R.string.dns_preset_quad9,
            descRes = R.string.dns_preset_quad9_desc,
            icon = "🛡️",
            ipv4 = "9.9.9.9",
            ipv6 = "2620:fe::fe"
        ),
        DnsPreset(
            id = "manual",
            nameRes = R.string.dns_preset_manual,
            descRes = R.string.dns_preset_manual_desc,
            icon = "✏️",
            ipv4 = "",
            ipv6 = ""
        )
    )

    fun show(
        context: Context,
        title: String,
        subtitle: String? = null,
        options: List<SheetOption>,
        onSelect: (SheetOption) -> Unit
    ) {
        val dialog = BottomSheetDialog(context, R.style.Theme_VpnShareProxy_BottomSheetDialog)
        val view = LayoutInflater.from(context).inflate(R.layout.dialog_selection_bottom_sheet, null)
        dialog.setContentView(view)

        val tvSheetTitle = view.findViewById<TextView>(R.id.tvSheetTitle)
        val tvSheetSubtitle = view.findViewById<TextView>(R.id.tvSheetSubtitle)
        val layoutOptionsContainer = view.findViewById<LinearLayout>(R.id.layoutOptionsContainer)
        val btnSheetCancel = view.findViewById<TextView>(R.id.btnSheetCancel)

        tvSheetTitle.text = title
        if (!subtitle.isNullOrBlank()) {
            tvSheetSubtitle.text = subtitle
            tvSheetSubtitle.visibility = View.VISIBLE
        } else {
            tvSheetSubtitle.visibility = View.GONE
        }

        val inflater = LayoutInflater.from(context)
        for (opt in options) {
            val itemView = inflater.inflate(R.layout.item_sheet_option, layoutOptionsContainer, false)
            val optionRoot = itemView.findViewById<LinearLayout>(R.id.optionRoot)
            val tvOptionIcon = itemView.findViewById<TextView>(R.id.tvOptionIcon)
            val tvOptionTitle = itemView.findViewById<TextView>(R.id.tvOptionTitle)
            val tvOptionSubtitle = itemView.findViewById<TextView>(R.id.tvOptionSubtitle)
            val tvOptionDetail = itemView.findViewById<TextView>(R.id.tvOptionDetail)
            val tvOptionSelectedBadge = itemView.findViewById<TextView>(R.id.tvOptionSelectedBadge)

            tvOptionIcon.text = opt.icon
            tvOptionTitle.text = opt.title

            if (!opt.subtitle.isNullOrBlank()) {
                tvOptionSubtitle.text = opt.subtitle
                tvOptionSubtitle.visibility = View.VISIBLE
            } else {
                tvOptionSubtitle.visibility = View.GONE
            }

            if (!opt.detail.isNullOrBlank()) {
                tvOptionDetail.text = opt.detail
                tvOptionDetail.visibility = View.VISIBLE
            } else {
                tvOptionDetail.visibility = View.GONE
            }

            if (opt.isSelected) {
                optionRoot.setBackgroundResource(R.drawable.bg_option_selected)
                tvOptionSelectedBadge.visibility = View.VISIBLE
            } else {
                optionRoot.setBackgroundResource(R.drawable.bg_option_idle)
                tvOptionSelectedBadge.visibility = View.GONE
            }

            optionRoot.setOnClickListener {
                dialog.dismiss()
                onSelect(opt)
            }

            layoutOptionsContainer.addView(itemView)
        }

        btnSheetCancel.setOnClickListener {
            dialog.dismiss()
        }

        dialog.show()
    }

    fun showThemeSelector(
        context: Context,
        currentTheme: String,
        onSelected: (String) -> Unit
    ) {
        val options = listOf(
            SheetOption(
                id = "system",
                icon = "🌗",
                title = context.getString(R.string.theme_system),
                subtitle = context.getString(R.string.theme_system_desc),
                isSelected = currentTheme != "light" && currentTheme != "dark"
            ),
            SheetOption(
                id = "light",
                icon = "☀️",
                title = context.getString(R.string.theme_light),
                subtitle = context.getString(R.string.theme_light_desc),
                isSelected = currentTheme == "light"
            ),
            SheetOption(
                id = "dark",
                icon = "🌙",
                title = context.getString(R.string.theme_dark),
                subtitle = context.getString(R.string.theme_dark_desc),
                isSelected = currentTheme == "dark"
            )
        )

        show(
            context = context,
            title = context.getString(R.string.theme_sheet_title),
            subtitle = context.getString(R.string.theme_sheet_subtitle),
            options = options
        ) { chosen ->
            onSelected(chosen.id)
        }
    }

    fun showLanguageSelector(
        context: Context,
        currentTag: String,
        onSelected: (LocaleListCompat) -> Unit
    ) {
        val isZh = currentTag.startsWith("zh")
        val isEn = currentTag.startsWith("en")
        val isSystem = !isZh && !isEn

        val options = listOf(
            SheetOption(
                id = "system",
                icon = "🌐",
                title = context.getString(R.string.lang_system),
                subtitle = context.getString(R.string.lang_system_desc),
                isSelected = isSystem
            ),
            SheetOption(
                id = "zh",
                icon = "🇨🇳",
                title = context.getString(R.string.lang_zh),
                subtitle = context.getString(R.string.lang_zh_desc),
                isSelected = isZh
            ),
            SheetOption(
                id = "en",
                icon = "🇬🇧",
                title = context.getString(R.string.lang_en),
                subtitle = context.getString(R.string.lang_en_desc),
                isSelected = isEn
            )
        )

        show(
            context = context,
            title = context.getString(R.string.lang_sheet_title),
            subtitle = context.getString(R.string.lang_sheet_subtitle),
            options = options
        ) { chosen ->
            val locales = when (chosen.id) {
                "zh" -> LocaleListCompat.forLanguageTags("zh-CN")
                "en" -> LocaleListCompat.forLanguageTags("en")
                else -> LocaleListCompat.getEmptyLocaleList()
            }
            onSelected(locales)
        }
    }

    fun showDnsPresetSelector(
        context: Context,
        currentV4: String,
        currentV6: String,
        onSelected: (DnsPreset) -> Unit
    ) {
        val options = DNS_PRESETS.map { preset ->
            val isCurrent = if (preset.id == "manual") {
                false
            } else {
                (preset.ipv4.isNotBlank() && preset.ipv4 == currentV4.trim()) ||
                (preset.ipv6.isNotBlank() && preset.ipv6 == currentV6.trim())
            }
            val detail = if (preset.ipv4.isNotBlank() && preset.ipv6.isNotBlank()) {
                "IPv4: ${preset.ipv4}  |  IPv6: ${preset.ipv6}"
            } else if (preset.ipv4.isNotBlank()) {
                "IPv4: ${preset.ipv4}"
            } else null

            SheetOption(
                id = preset.id,
                icon = preset.icon,
                title = context.getString(preset.nameRes),
                subtitle = context.getString(preset.descRes),
                detail = detail,
                isSelected = isCurrent
            )
        }

        show(
            context = context,
            title = context.getString(R.string.dns_preset_title),
            subtitle = context.getString(R.string.dns_preset_subtitle),
            options = options
        ) { chosen ->
            val match = DNS_PRESETS.firstOrNull { it.id == chosen.id } ?: DNS_PRESETS.first()
            onSelected(match)
        }
    }
}
