package com.vpnshareproxy.app

import android.content.Intent
import android.os.Bundle
import android.view.View
import android.widget.EditText
import android.widget.TextView
import android.widget.Toast
import androidx.appcompat.app.AlertDialog
import androidx.appcompat.app.AppCompatActivity
import androidx.appcompat.app.AppCompatDelegate
import androidx.core.content.ContextCompat
import androidx.core.os.LocaleListCompat
import androidx.lifecycle.lifecycleScope
import androidx.recyclerview.widget.LinearLayoutManager
import androidx.recyclerview.widget.RecyclerView
import androidx.swiperefreshlayout.widget.SwipeRefreshLayout
import com.vpnshareproxy.app.data.ConnectedDevice
import com.vpnshareproxy.app.data.DaemonStatus
import com.vpnshareproxy.app.data.GatewayConfig
import com.vpnshareproxy.app.data.NetIface
import com.vpnshareproxy.app.data.SocketClient
import com.vpnshareproxy.app.ui.DeviceAdapter
import com.vpnshareproxy.app.ui.DnsPreset
import com.vpnshareproxy.app.ui.InterfaceAdapter
import com.vpnshareproxy.app.ui.SelectionBottomSheet
import com.vpnshareproxy.app.util.IpFormats
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch

class MainActivity : AppCompatActivity() {

    private lateinit var tvStatusBadge: TextView
    private lateinit var tvVpnIface: TextView
    private lateinit var tvDeviceCount: TextView
    private lateinit var rvDevices: RecyclerView
    private lateinit var rvIfaces: RecyclerView
    private lateinit var sectionManagedIfaces: View
    private lateinit var sectionDevices: View
    private lateinit var swipeRefresh: SwipeRefreshLayout
    private lateinit var btnThemeToggle: TextView
    private lateinit var btnLangToggle: TextView
    private lateinit var btnReconcile: TextView
    private lateinit var btnHelp: TextView
    private lateinit var tvEmptyState: TextView
    private lateinit var tvModeDesc: TextView
    private lateinit var tvModeDeviceHint: TextView
    private lateinit var btnModeGlobal: TextView
    private lateinit var btnModeAllow: TextView
    private lateinit var btnModeBlock: TextView
    private lateinit var btnStackV4: TextView
    private lateinit var btnStackV6: TextView
    private lateinit var tvStackHint: TextView
    private lateinit var btnDnsOff: TextView
    private lateinit var btnDnsAuto: TextView
    private lateinit var btnDnsCustom: TextView
    private lateinit var tvDnsHint: TextView
    private lateinit var etDnsCustomV4: EditText
    private lateinit var etDnsCustomV6: EditText
    private lateinit var btnDnsApply: TextView
    private lateinit var layoutDnsPresetPicker: View
    private lateinit var tvDnsPresetTitle: TextView
    private lateinit var scrollDnsPresets: View

    private lateinit var adapter: DeviceAdapter
    private lateinit var ifaceAdapter: InterfaceAdapter
    private val socketClient = SocketClient.instance
    private var eventStreamJob: Job? = null
    private var currentMode: String = "global"
    private var enableIpv4: Boolean = true
    private var enableIpv6: Boolean = false
    private var dnsRedirect: String = "off"
    private var modeSwitchInFlight = false
    private var stackSwitchInFlight = false
    private var dnsSwitchInFlight = false
    private var ifaceSwitchInFlight = false
    private val managedIfaces = linkedSetOf<String>()

    override fun onCreate(savedInstanceState: Bundle?) {
        applyTheme(getSavedTheme())
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)

        initViews()
        setupRecyclerView()
        setupListeners()
        loadInitialData()
        startEventStream()
    }

    private fun initViews() {
        tvStatusBadge = findViewById(R.id.tvStatusBadge)
        tvVpnIface = findViewById(R.id.tvVpnIface)
        tvDeviceCount = findViewById(R.id.tvDeviceCount)
        rvDevices = findViewById(R.id.rvDevices)
        swipeRefresh = findViewById(R.id.swipeRefresh)
        btnThemeToggle = findViewById(R.id.btnThemeToggle)
        btnLangToggle = findViewById(R.id.btnLangToggle)
        btnReconcile = findViewById(R.id.btnReconcile)
        btnHelp = findViewById(R.id.btnHelp)
        tvEmptyState = findViewById(R.id.tvEmptyState)
        tvModeDesc = findViewById(R.id.tvModeDesc)
        tvModeDeviceHint = findViewById(R.id.tvModeDeviceHint)
        btnModeGlobal = findViewById(R.id.btnModeGlobal)
        btnModeAllow = findViewById(R.id.btnModeAllow)
        btnModeBlock = findViewById(R.id.btnModeBlock)
        btnStackV4 = findViewById(R.id.btnStackV4)
        btnStackV6 = findViewById(R.id.btnStackV6)
        tvStackHint = findViewById(R.id.tvStackHint)
        btnDnsOff = findViewById(R.id.btnDnsOff)
        btnDnsAuto = findViewById(R.id.btnDnsAuto)
        btnDnsCustom = findViewById(R.id.btnDnsCustom)
        tvDnsHint = findViewById(R.id.tvDnsHint)
        etDnsCustomV4 = findViewById(R.id.etDnsCustomV4)
        etDnsCustomV6 = findViewById(R.id.etDnsCustomV6)
        btnDnsApply = findViewById(R.id.btnDnsApply)
        layoutDnsPresetPicker = findViewById(R.id.layoutDnsPresetPicker)
        tvDnsPresetTitle = findViewById(R.id.tvDnsPresetTitle)
        scrollDnsPresets = findViewById(R.id.scrollDnsPresets)
        rvIfaces = findViewById(R.id.rvIfaces)
        sectionManagedIfaces = findViewById(R.id.sectionManagedIfaces)
        sectionDevices = findViewById(R.id.sectionDevices)

        updateThemeButtonIcon(getSavedTheme())
        updateLanguageButtonLabel()
    }

    private fun setupRecyclerView() {
        adapter = DeviceAdapter(mutableListOf(), currentMode) { device, newPolicy ->
            handleDevicePolicyChange(device, newPolicy)
        }
        rvDevices.layoutManager = LinearLayoutManager(this)
        rvDevices.adapter = adapter

        ifaceAdapter = InterfaceAdapter(mutableListOf()) { iface ->
            toggleManagedIface(iface)
        }
        rvIfaces.layoutManager = LinearLayoutManager(this)
        rvIfaces.adapter = ifaceAdapter
    }

    private fun setupListeners() {
        swipeRefresh.setOnRefreshListener { loadInitialData() }

        btnThemeToggle.setOnClickListener { showThemeDialog() }
        btnLangToggle.setOnClickListener { showLanguageDialog() }

        btnReconcile.setOnClickListener {
            lifecycleScope.launch {
                btnReconcile.isEnabled = false
                val success = socketClient.reconcileNow()
                btnReconcile.isEnabled = true
                if (success) {
                    Toast.makeText(this@MainActivity, getString(R.string.rule_reconciled), Toast.LENGTH_SHORT).show()
                    loadInitialData()
                } else {
                    Toast.makeText(this@MainActivity, getString(R.string.daemon_unresponsive), Toast.LENGTH_SHORT).show()
                }
            }
        }

        btnHelp.setOnClickListener {
            startActivity(Intent(this, HelpActivity::class.java))
        }

        btnModeGlobal.setOnClickListener { switchMode("global") }
        btnModeAllow.setOnClickListener { switchMode("mac_allowlist") }
        btnModeBlock.setOnClickListener { switchMode("mac_blocklist") }

        btnStackV4.setOnClickListener { toggleStack(ipv4 = !enableIpv4, ipv6 = null) }
        btnStackV6.setOnClickListener { toggleStack(ipv4 = null, ipv6 = !enableIpv6) }

        btnDnsOff.setOnClickListener { switchDns("off") }
        btnDnsAuto.setOnClickListener { switchDns("auto") }
        btnDnsCustom.setOnClickListener { switchDns("custom") }
        btnDnsApply.setOnClickListener { applyCustomDns() }

        layoutDnsPresetPicker.setOnClickListener {
            SelectionBottomSheet.showDnsPresetSelector(
                this,
                etDnsCustomV4.text.toString(),
                etDnsCustomV6.text.toString()
            ) { preset ->
                applyDnsPreset(preset)
            }
        }

        findViewById<View>(R.id.chipDnsAli).setOnClickListener {
            SelectionBottomSheet.DNS_PRESETS.find { it.id == "ali" }?.let { applyDnsPreset(it) }
        }
        findViewById<View>(R.id.chipDnsTencent).setOnClickListener {
            SelectionBottomSheet.DNS_PRESETS.find { it.id == "tencent" }?.let { applyDnsPreset(it) }
        }
        findViewById<View>(R.id.chipDnsCloudflare).setOnClickListener {
            SelectionBottomSheet.DNS_PRESETS.find { it.id == "cloudflare" }?.let { applyDnsPreset(it) }
        }
        findViewById<View>(R.id.chipDnsGoogle).setOnClickListener {
            SelectionBottomSheet.DNS_PRESETS.find { it.id == "google" }?.let { applyDnsPreset(it) }
        }
        findViewById<View>(R.id.chipDns114).setOnClickListener {
            SelectionBottomSheet.DNS_PRESETS.find { it.id == "114" }?.let { applyDnsPreset(it) }
        }
        findViewById<View>(R.id.chipDnsQuad9).setOnClickListener {
            SelectionBottomSheet.DNS_PRESETS.find { it.id == "quad9" }?.let { applyDnsPreset(it) }
        }
    }

    private fun applyDnsPreset(preset: DnsPreset) {
        tvDnsPresetTitle.text = getString(preset.nameRes)
        etDnsCustomV4.setText(preset.ipv4)
        etDnsCustomV6.setText(preset.ipv6)
        if (preset.ipv4.isNotBlank()) {
            Toast.makeText(this, "${getString(preset.nameRes)}: ${preset.ipv4}", Toast.LENGTH_SHORT).show()
        }
    }

    private fun getSavedTheme(): String {
        val prefs = getSharedPreferences("tetherflow_settings", MODE_PRIVATE)
        return prefs.getString("theme_mode", "system") ?: "system"
    }

    private fun applyTheme(themeMode: String) {
        when (themeMode) {
            "light" -> AppCompatDelegate.setDefaultNightMode(AppCompatDelegate.MODE_NIGHT_NO)
            "dark" -> AppCompatDelegate.setDefaultNightMode(AppCompatDelegate.MODE_NIGHT_YES)
            else -> AppCompatDelegate.setDefaultNightMode(AppCompatDelegate.MODE_NIGHT_FOLLOW_SYSTEM)
        }
    }

    private fun updateThemeButtonIcon(themeMode: String) {
        btnThemeToggle.text = when (themeMode) {
            "light" -> "☀️"
            "dark" -> "🌙"
            else -> "🌗"
        }
    }

    private fun showThemeDialog() {
        SelectionBottomSheet.showThemeSelector(this, getSavedTheme()) { chosen ->
            getSharedPreferences("tetherflow_settings", MODE_PRIVATE)
                .edit()
                .putString("theme_mode", chosen)
                .apply()
            applyTheme(chosen)
            updateThemeButtonIcon(chosen)
        }
    }

    private fun updateLanguageButtonLabel() {
        val currentLocales = AppCompatDelegate.getApplicationLocales()
        val currentTag = if (currentLocales.isEmpty) "" else currentLocales[0]?.toLanguageTag().orEmpty()
        btnLangToggle.text = when {
            currentTag.startsWith("zh") -> "中"
            currentTag.startsWith("en") -> "EN"
            else -> "🌐"
        }
    }

    private fun showLanguageDialog() {
        val currentLocales = AppCompatDelegate.getApplicationLocales()
        val currentTag = if (currentLocales.isEmpty) "" else currentLocales[0]?.toLanguageTag().orEmpty()
        SelectionBottomSheet.showLanguageSelector(this, currentTag) { newLocales ->
            AppCompatDelegate.setApplicationLocales(newLocales)
            updateLanguageButtonLabel()
        }
    }

    private fun toggleStack(ipv4: Boolean?, ipv6: Boolean?) {
        if (stackSwitchInFlight) return
        val nextV4 = ipv4 ?: enableIpv4
        val nextV6 = ipv6 ?: enableIpv6
        if (nextV4 == enableIpv4 && nextV6 == enableIpv6) return

        stackSwitchInFlight = true
        paintStackButtons(nextV4, nextV6)

        lifecycleScope.launch {
            val ok = socketClient.setStackEnabled(ipv4 = ipv4, ipv6 = ipv6)
            stackSwitchInFlight = false
            if (ok) {
                enableIpv4 = nextV4
                enableIpv6 = nextV6
                Toast.makeText(
                    this@MainActivity,
                    "IPv4 ${onOff(enableIpv4)} / IPv6 ${onOff(enableIpv6)}",
                    Toast.LENGTH_SHORT
                ).show()
                loadInitialData()
            } else {
                paintStackButtons(enableIpv4, enableIpv6)
                Toast.makeText(this@MainActivity, getString(R.string.switch_failed), Toast.LENGTH_SHORT).show()
            }
        }
    }

    private fun onOff(on: Boolean) = if (on) "ON" else "OFF"

    private fun switchDns(mode: String) {
        if (dnsSwitchInFlight) return
        if (mode == dnsRedirect && mode != "custom") {
            paintDnsUi(mode)
            return
        }
        if (mode == "custom") {
            ensureCustomDnsDefaultsInFields()
            if (!validateCustomDnsFields()) {
                paintDnsUi("custom")
                return
            }
        }
        dnsSwitchInFlight = true
        paintDnsUi(mode)
        lifecycleScope.launch {
            val v4 = etDnsCustomV4.text?.toString()
            val v6 = etDnsCustomV6.text?.toString()
            val ok = socketClient.setDnsRedirect(mode, v4, v6)
            dnsSwitchInFlight = false
            if (ok) {
                dnsRedirect = mode
                Toast.makeText(
                    this@MainActivity,
                    when (mode) {
                        "auto" -> "DNS: Auto"
                        "custom" -> "DNS: Custom"
                        else -> "DNS: Off"
                    },
                    Toast.LENGTH_SHORT
                ).show()
                loadInitialData()
            } else {
                paintDnsUi(dnsRedirect)
                Toast.makeText(this@MainActivity, getString(R.string.switch_failed), Toast.LENGTH_SHORT).show()
            }
        }
    }

    private fun applyCustomDns() {
        if (dnsSwitchInFlight) return
        ensureCustomDnsDefaultsInFields()
        if (!validateCustomDnsFields()) return
        val v4 = etDnsCustomV4.text?.toString()?.trim().orEmpty()
        val v6 = etDnsCustomV6.text?.toString()?.trim().orEmpty()
        dnsSwitchInFlight = true
        lifecycleScope.launch {
            val ok = socketClient.setDnsRedirect("custom", v4, v6)
            dnsSwitchInFlight = false
            if (ok) {
                dnsRedirect = "custom"
                Toast.makeText(this@MainActivity, "DNS: $v4 / $v6", Toast.LENGTH_SHORT).show()
                loadInitialData()
            } else {
                Toast.makeText(this@MainActivity, getString(R.string.dns_apply_failed), Toast.LENGTH_SHORT).show()
            }
        }
    }

    private fun validateCustomDnsFields(): Boolean {
        val v4 = etDnsCustomV4.text?.toString()?.trim().orEmpty()
        val v6 = etDnsCustomV6.text?.toString()?.trim().orEmpty()
        var ok = true
        if (!IpFormats.isIpv4(v4)) {
            etDnsCustomV4.error = getString(R.string.dns_invalid_v4)
            ok = false
        } else {
            etDnsCustomV4.error = null
        }
        if (!IpFormats.isIpv6(v6)) {
            etDnsCustomV6.error = getString(R.string.dns_invalid_v6)
            ok = false
        } else {
            etDnsCustomV6.error = null
        }
        return ok
    }

    private fun ensureCustomDnsDefaultsInFields() {
        if (etDnsCustomV4.text.isNullOrBlank()) {
            etDnsCustomV4.setText(GatewayConfig.DEFAULT_DNS_V4)
        }
        if (etDnsCustomV6.text.isNullOrBlank()) {
            etDnsCustomV6.setText(GatewayConfig.DEFAULT_DNS_V6)
        }
    }

    private fun paintDnsUi(mode: String) {
        styleModeButton(btnDnsOff, mode == "off")
        styleModeButton(btnDnsAuto, mode == "auto")
        styleModeButton(btnDnsCustom, mode == "custom")
        val custom = mode == "custom"
        layoutDnsPresetPicker.visibility = if (custom) View.VISIBLE else View.GONE
        scrollDnsPresets.visibility = if (custom) View.VISIBLE else View.GONE
        etDnsCustomV4.visibility = if (custom) View.VISIBLE else View.GONE
        etDnsCustomV6.visibility = if (custom) View.VISIBLE else View.GONE
        btnDnsApply.visibility = if (custom) View.VISIBLE else View.GONE
        when (mode) {
            "auto" -> tvDnsHint.setText(R.string.dns_auto_hint)
            "custom" -> tvDnsHint.setText(R.string.dns_custom_mode_hint)
            else -> tvDnsHint.setText(R.string.dns_off_hint)
        }
    }

    private fun switchMode(targetMode: String) {
        if (modeSwitchInFlight || targetMode == currentMode) {
            paintModeButtons(currentMode)
            return
        }
        modeSwitchInFlight = true
        paintModeButtons(targetMode)
        updateModeCopy(targetMode)

        lifecycleScope.launch {
            val ok = socketClient.setProxyMode(targetMode)
            modeSwitchInFlight = false
            if (ok) {
                currentMode = targetMode
                Toast.makeText(
                    this@MainActivity,
                    when (targetMode) {
                        "mac_allowlist" -> getString(R.string.mode_switched_allow)
                        "mac_blocklist" -> getString(R.string.mode_switched_block)
                        else -> getString(R.string.mode_switched_global)
                    },
                    Toast.LENGTH_SHORT
                ).show()
                loadInitialData()
            } else {
                paintModeButtons(currentMode)
                updateModeCopy(currentMode)
                Toast.makeText(this@MainActivity, getString(R.string.switch_failed), Toast.LENGTH_SHORT).show()
            }
        }
    }

    private fun loadInitialData() {
        lifecycleScope.launch {
            swipeRefresh.isRefreshing = true

            val status = socketClient.getStatus()
            updateStatusUi(status)

            val config = socketClient.getConfig()
            if (config != null) {
                currentMode = config.proxyMode.lowercase()
                enableIpv4 = config.enableIpv4
                enableIpv6 = config.enableIpv6
                dnsRedirect = config.dnsRedirect.lowercase().ifBlank { "off" }
                managedIfaces.clear()
                managedIfaces.addAll(config.managedIfaces)
                paintModeButtons(currentMode)
                updateModeCopy(currentMode)
                paintStackButtons(enableIpv4, enableIpv6)
                paintDnsUi(dnsRedirect)
                etDnsCustomV4.setText(config.customDnsV4 ?: GatewayConfig.DEFAULT_DNS_V4)
                etDnsCustomV6.setText(config.customDnsV6 ?: GatewayConfig.DEFAULT_DNS_V6)
            }

            val ifaces = socketClient.listInterfaces()
            ifaceAdapter.update(ifaces)

            val devices = socketClient.getDevices()
            updateDevicesUi(devices, currentMode)

            swipeRefresh.isRefreshing = false
        }
    }

    private fun paintStackButtons(v4: Boolean, v6: Boolean) {
        styleStackButton(btnStackV4, v4)
        styleStackButton(btnStackV6, v6)
        tvStackHint.text = getString(R.string.stack_hint)
    }

    private fun styleStackButton(btn: TextView, selected: Boolean) {
        if (selected) {
            btn.setBackgroundResource(R.drawable.bg_mode_selected)
            btn.setTextColor(ContextCompat.getColor(this, R.color.mode_selected_fg))
        } else {
            btn.setBackgroundResource(R.drawable.bg_chip_idle)
            btn.setTextColor(ContextCompat.getColor(this, R.color.mode_idle_fg))
        }
    }

    private fun paintModeButtons(mode: String) {
        styleModeButton(btnModeGlobal, mode == "global")
        styleModeButton(btnModeAllow, mode == "mac_allowlist")
        styleModeButton(btnModeBlock, mode == "mac_blocklist")
    }

    private fun styleModeButton(btn: TextView, selected: Boolean) {
        if (selected) {
            btn.setBackgroundResource(R.drawable.bg_mode_selected)
            btn.setTextColor(ContextCompat.getColor(this, R.color.mode_selected_fg))
        } else {
            btn.setBackgroundResource(R.drawable.bg_mode_idle)
            btn.setTextColor(ContextCompat.getColor(this, R.color.mode_idle_fg))
        }
    }

    private fun updateModeCopy(mode: String) {
        val listMode = mode == "mac_allowlist" || mode == "mac_blocklist"
        sectionManagedIfaces.visibility = if (listMode) View.VISIBLE else View.GONE
        sectionDevices.visibility = if (listMode) View.VISIBLE else View.GONE

        when (mode) {
            "mac_allowlist" -> tvModeDesc.setText(R.string.mode_allow_hint)
            "mac_blocklist" -> tvModeDesc.setText(R.string.mode_block_hint)
            else -> tvModeDesc.setText(R.string.mode_global_hint)
        }

        if (listMode && managedIfaces.isEmpty()) {
            tvModeDeviceHint.visibility = View.VISIBLE
            tvModeDeviceHint.setText(R.string.empty_managed_hint)
        } else {
            tvModeDeviceHint.visibility = View.GONE
        }
    }

    private fun updateDevicesUi(devices: List<ConnectedDevice>, mode: String) {
        val listMode = mode == "mac_allowlist" || mode == "mac_blocklist"
        if (!listMode) {
            adapter.updateData(emptyList(), mode)
            return
        }
        adapter.updateData(devices, mode)
        if (managedIfaces.isEmpty()) {
            tvModeDeviceHint.visibility = View.VISIBLE
            tvEmptyState.visibility = View.GONE
            rvDevices.visibility = View.GONE
        } else if (devices.isEmpty()) {
            tvModeDeviceHint.visibility = View.GONE
            tvEmptyState.visibility = View.VISIBLE
            rvDevices.visibility = View.GONE
        } else {
            tvModeDeviceHint.visibility = View.GONE
            tvEmptyState.visibility = View.GONE
            rvDevices.visibility = View.VISIBLE
        }
    }

    private fun updateStatusUi(status: DaemonStatus?) {
        if (status == null) {
            tvStatusBadge.text = getString(R.string.status_offline)
            tvStatusBadge.setBackgroundResource(R.drawable.bg_status_off)
            tvStatusBadge.setTextColor(ContextCompat.getColor(this, R.color.danger))
            tvVpnIface.text = "tun —"
            tvDeviceCount.text = getString(R.string.device_count_format, 0)
            return
        }

        when (status.state.lowercase()) {
            "active" -> {
                tvStatusBadge.text = getString(R.string.status_active)
                tvStatusBadge.setBackgroundResource(R.drawable.bg_status_pill)
                tvStatusBadge.setTextColor(ContextCompat.getColor(this, R.color.accent))
            }
            "waiting_vpn" -> {
                tvStatusBadge.text = getString(R.string.status_waiting)
                tvStatusBadge.setBackgroundResource(R.drawable.bg_status_wait)
                tvStatusBadge.setTextColor(ContextCompat.getColor(this, R.color.warn))
            }
            else -> {
                tvStatusBadge.text = getString(R.string.status_standby)
                tvStatusBadge.setBackgroundResource(R.drawable.bg_status_off)
                tvStatusBadge.setTextColor(ContextCompat.getColor(this, R.color.danger))
            }
        }

        val tun = status.tunInterface ?: "—"
        val table = status.tableId?.toString() ?: "—"
        val stacks = buildString {
            append(if (status.ipv4Active) "v4" else "—")
            append("/")
            append(if (status.ipv6Active) "v6" else "—")
        }
        val tableStr = getString(R.string.table_format, table)
        tvVpnIface.text = "$tun ($tableStr · $stacks)"
        tvDeviceCount.text = getString(R.string.device_count_format, status.clientCount)
    }

    private fun toggleManagedIface(iface: NetIface) {
        if (ifaceSwitchInFlight) return
        ifaceSwitchInFlight = true
        val next = managedIfaces.toMutableSet()
        if (next.contains(iface.name)) next.remove(iface.name) else next.add(iface.name)

        lifecycleScope.launch {
            val ok = socketClient.setManagedIfaces(next)
            ifaceSwitchInFlight = false
            if (ok) {
                managedIfaces.clear()
                managedIfaces.addAll(next)
                val msg = if (next.contains(iface.name)) {
                    getString(R.string.iface_managed_format, iface.name)
                } else {
                    getString(R.string.iface_unmanaged_format, iface.name)
                }
                Toast.makeText(this@MainActivity, msg, Toast.LENGTH_SHORT).show()
                loadInitialData()
            } else {
                Toast.makeText(this@MainActivity, getString(R.string.iface_set_failed), Toast.LENGTH_SHORT).show()
            }
        }
    }

    private fun handleDevicePolicyChange(device: ConnectedDevice, newPolicy: String) {
        lifecycleScope.launch {
            val success = socketClient.setDevicePolicy(device.mac, newPolicy)
            if (success) {
                loadInitialData()
            } else {
                Toast.makeText(this@MainActivity, getString(R.string.device_set_failed), Toast.LENGTH_SHORT).show()
                loadInitialData()
            }
        }
    }

    private fun startEventStream() {
        eventStreamJob?.cancel()
        eventStreamJob = lifecycleScope.launch {
            socketClient.subscribeEvents().collect { eventJson ->
                when (eventJson.optString("event")) {
                    "vpn_changed", "reconcile_finished", "state_changed",
                    "device_connected", "device_disconnected", "device_policy_changed" -> {
                        loadInitialData()
                    }
                }
            }
        }
    }

    override fun onDestroy() {
        super.onDestroy()
        eventStreamJob?.cancel()
    }
}
