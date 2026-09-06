package com.vpnshareproxy.app.data

import org.json.JSONObject

data class DaemonStatus(
    val state: String,
    val tunInterface: String?,
    val tableId: Int?,
    val hotspotInterfaces: List<String>,
    val ipv4Active: Boolean,
    val ipv6Active: Boolean,
    val clientCount: Int,
    val uptimeSecs: Long,
    val version: String
) {
    companion object {
        fun fromJson(json: JSONObject): DaemonStatus {
            val ifaces = mutableListOf<String>()
            val ifaceArray = json.optJSONArray("hotspot_interfaces")
            if (ifaceArray != null) {
                for (i in 0 until ifaceArray.length()) {
                    ifaces.add(ifaceArray.getString(i))
                }
            }

            return DaemonStatus(
                state = json.optString("state", "unknown"),
                tunInterface = if (json.has("tun_interface") && !json.isNull("tun_interface")) json.getString("tun_interface") else null,
                tableId = if (json.has("table_id") && !json.isNull("table_id")) json.getInt("table_id") else null,
                hotspotInterfaces = ifaces,
                ipv4Active = json.optBoolean("ipv4_active", false),
                ipv6Active = json.optBoolean("ipv6_active", false),
                clientCount = json.optInt("client_count", 0),
                uptimeSecs = json.optLong("uptime_secs", 0L),
                version = json.optString("version", "0.1.0")
            )
        }
    }
}

data class ConnectedDevice(
    val ip: String,
    val mac: String,
    val iface: String,
    val hostname: String?,
    var policy: String, // "allowed", "bypass", "default"
    val firstSeen: Long,
    val lastSeen: Long
) {
    val displayName: String
        get() = hostname ?: "设备 ($ip)"

    val isBypassed: Boolean
        get() = policy.equals("bypass", ignoreCase = true) || policy.equals("block", ignoreCase = true)

    companion object {
        fun fromJson(json: JSONObject): ConnectedDevice {
            return ConnectedDevice(
                ip = json.optString("ip", ""),
                mac = json.optString("mac", "").uppercase(),
                iface = json.optString("iface", ""),
                hostname = if (json.has("hostname") && !json.isNull("hostname")) json.getString("hostname") else null,
                policy = json.optString("policy", "allowed"),
                firstSeen = json.optLong("first_seen", 0L),
                lastSeen = json.optLong("last_seen", 0L)
            )
        }
    }
}

data class NetIface(
    val name: String,
    val operstate: String,
    val recommended: Boolean,
    val hint: String,
    var managed: Boolean
) {
    companion object {
        fun fromJson(json: JSONObject): NetIface {
            return NetIface(
                name = json.optString("name", ""),
                operstate = json.optString("operstate", "unknown"),
                recommended = json.optBoolean("recommended", false),
                hint = json.optString("hint", ""),
                managed = json.optBoolean("managed", false)
            )
        }
    }
}

data class GatewayConfig(
    var enableIpv4: Boolean = true,
    var enableIpv6: Boolean = false,
    var dnsRedirect: String = "off",
    var customDnsV4: String? = null,
    var customDnsV6: String? = null,
    var proxyMode: String = "global",
    val macAllowList: MutableList<String> = mutableListOf(),
    val macBlockList: MutableList<String> = mutableListOf(),
    val managedIfaces: MutableList<String> = mutableListOf()
) {
    fun toJson(): JSONObject {
        return JSONObject().apply {
            put("enable_ipv4", enableIpv4)
            put("enable_ipv6", enableIpv6)
            put("dns_redirect", dnsRedirect)
            if (customDnsV4.isNullOrBlank()) put("custom_dns_v4", JSONObject.NULL)
            else put("custom_dns_v4", customDnsV4)
            if (customDnsV6.isNullOrBlank()) put("custom_dns_v6", JSONObject.NULL)
            else put("custom_dns_v6", customDnsV6)
            put("proxy_mode", proxyMode)
            put("mac_allow_list", org.json.JSONArray(macAllowList))
            put("mac_block_list", org.json.JSONArray(macBlockList))
            put("managed_ifaces", org.json.JSONArray(managedIfaces))
        }
    }

    companion object {
        const val DEFAULT_DNS_V4 = "8.8.8.8"
        const val DEFAULT_DNS_V6 = "2001:4860:4860::8888"

        /** JSON null must not become the literal string "null" (Android JSONObject.optString quirk). */
        private fun optNullableString(json: JSONObject, key: String): String? {
            if (!json.has(key) || json.isNull(key)) return null
            return json.optString(key, "").trim().ifBlank { null }
        }

        fun fromJson(json: JSONObject): GatewayConfig {
            val allow = mutableListOf<String>()
            val allowArr = json.optJSONArray("mac_allow_list")
            if (allowArr != null) {
                for (i in 0 until allowArr.length()) allow.add(allowArr.getString(i).uppercase())
            }
            val block = mutableListOf<String>()
            val blockArr = json.optJSONArray("mac_block_list")
            if (blockArr != null) {
                for (i in 0 until blockArr.length()) block.add(blockArr.getString(i).uppercase())
            }
            val managed = mutableListOf<String>()
            val managedArr = json.optJSONArray("managed_ifaces")
            if (managedArr != null) {
                for (i in 0 until managedArr.length()) managed.add(managedArr.getString(i))
            }
            return GatewayConfig(
                enableIpv4 = json.optBoolean("enable_ipv4", true),
                enableIpv6 = json.optBoolean("enable_ipv6", false),
                dnsRedirect = json.optString("dns_redirect", "off"),
                customDnsV4 = optNullableString(json, "custom_dns_v4"),
                customDnsV6 = optNullableString(json, "custom_dns_v6"),
                proxyMode = json.optString("proxy_mode", "global"),
                macAllowList = allow,
                macBlockList = block,
                managedIfaces = managed
            )
        }
    }
}
