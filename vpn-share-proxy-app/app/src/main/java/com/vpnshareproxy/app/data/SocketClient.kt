package com.vpnshareproxy.app.data

import android.net.LocalSocket
import android.net.LocalSocketAddress
import android.util.Log
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.flowOn
import kotlinx.coroutines.isActive
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject
import java.io.BufferedReader
import java.io.BufferedWriter
import java.io.InputStreamReader
import java.io.OutputStreamWriter
import java.util.concurrent.atomic.AtomicInteger

class SocketClient private constructor() {
    companion object {
        private const val TAG = "VspSocketClient"
        private const val ABSTRACT_SOCKET_NAME = "vpn_share_proxy.sock"

        val instance: SocketClient by lazy { SocketClient() }
    }

    private val reqId = AtomicInteger(1)

    private fun connectSocket(): LocalSocket {
        val socket = LocalSocket()
        val abstractAddr =
            LocalSocketAddress(ABSTRACT_SOCKET_NAME, LocalSocketAddress.Namespace.ABSTRACT)
        socket.connect(abstractAddr)
        return socket
    }

    private suspend fun sendRpc(action: String, params: JSONObject = JSONObject()): JSONObject? = withContext(Dispatchers.IO) {
        var socket: LocalSocket? = null
        try {
            socket = connectSocket()
            val writer = BufferedWriter(OutputStreamWriter(socket.outputStream, Charsets.UTF_8))
            val reader = BufferedReader(InputStreamReader(socket.inputStream, Charsets.UTF_8))

            val id = reqId.getAndIncrement()
            val request = JSONObject().apply {
                put("id", id)
                put("action", action)
                put("params", params)
            }

            writer.write(request.toString() + "\n")
            writer.flush()

            val responseLine = reader.readLine()
            if (responseLine != null) {
                return@withContext JSONObject(responseLine)
            }
        } catch (e: Exception) {
            Log.w(TAG, "sendRpc '$action' error: ${e.message}")
        } finally {
            try { socket?.close() } catch (_: Exception) {}
        }
        null
    }

    suspend fun getStatus(): DaemonStatus? = withContext(Dispatchers.IO) {
        val resp = sendRpc("get_status")
        if (resp != null && resp.optInt("code", -1) == 0) {
            val data = resp.optJSONObject("data")
            if (data != null) {
                return@withContext DaemonStatus.fromJson(data)
            }
        }
        null
    }

    suspend fun getDevices(): List<ConnectedDevice> = withContext(Dispatchers.IO) {
        val devices = mutableListOf<ConnectedDevice>()
        val resp = sendRpc("list_devices")
        if (resp != null && resp.optInt("code", -1) == 0) {
            val array = resp.optJSONArray("data")
            if (array != null) {
                for (i in 0 until array.length()) {
                    val obj = array.getJSONObject(i)
                    devices.add(ConnectedDevice.fromJson(obj))
                }
            }
        }
        devices
    }

    suspend fun setDevicePolicy(mac: String, policy: String): Boolean = withContext(Dispatchers.IO) {
        val params = JSONObject().apply {
            put("mac", mac)
            put("policy", policy)
        }
        val resp = sendRpc("set_device_policy", params)
        resp?.optInt("code", -1) == 0
    }

    suspend fun reconcileNow(): Boolean = withContext(Dispatchers.IO) {
        val resp = sendRpc("reconcile_now")
        resp?.optInt("code", -1) == 0
    }

    suspend fun getConfig(): GatewayConfig? = withContext(Dispatchers.IO) {
        val resp = sendRpc("get_config")
        if (resp != null && resp.optInt("code", -1) == 0) {
            val data = resp.optJSONObject("data")
            if (data != null) {
                return@withContext GatewayConfig.fromJson(data)
            }
        }
        null
    }

    suspend fun setProxyMode(mode: String): Boolean = withContext(Dispatchers.IO) {
        val params = JSONObject().apply {
            put("mode", mode)
        }
        val resp = sendRpc("set_proxy_mode", params)
        resp?.optInt("code", -1) == 0
    }

    suspend fun setConfig(config: GatewayConfig): Boolean = withContext(Dispatchers.IO) {
        val resp = sendRpc("set_config", config.toJson())
        resp?.optInt("code", -1) == 0
    }

    /** Patch enable_ipv4 / enable_ipv6 via full set_config. */
    suspend fun setStackEnabled(ipv4: Boolean? = null, ipv6: Boolean? = null): Boolean =
        withContext(Dispatchers.IO) {
            val current = getConfig() ?: return@withContext false
            ipv4?.let { current.enableIpv4 = it }
            ipv6?.let { current.enableIpv6 = it }
            setConfig(current)
        }

    suspend fun setDnsRedirect(
        mode: String,
        customDnsV4: String? = null,
        customDnsV6: String? = null
    ): Boolean = withContext(Dispatchers.IO) {
        val current = getConfig() ?: return@withContext false
        current.dnsRedirect = mode
        if (mode == "custom") {
            val v4 = customDnsV4?.trim()?.takeIf { it.isNotEmpty() } ?: GatewayConfig.DEFAULT_DNS_V4
            val v6 = customDnsV6?.trim()?.takeIf { it.isNotEmpty() } ?: GatewayConfig.DEFAULT_DNS_V6
            if (!com.vpnshareproxy.app.util.IpFormats.isIpv4(v4) || !com.vpnshareproxy.app.util.IpFormats.isIpv6(v6)) {
                Log.w(TAG, "reject custom DNS: bad literal v4=$v4 v6=$v6")
                return@withContext false
            }
            current.customDnsV4 = v4
            current.customDnsV6 = v6
        }
        setConfig(current)
    }

    suspend fun listInterfaces(): List<NetIface> = withContext(Dispatchers.IO) {
        val out = mutableListOf<NetIface>()
        val resp = sendRpc("list_interfaces")
        if (resp != null && resp.optInt("code", -1) == 0) {
            val arr = resp.optJSONArray("data")
            if (arr != null) {
                for (i in 0 until arr.length()) {
                    out.add(NetIface.fromJson(arr.getJSONObject(i)))
                }
            }
        }
        out
    }

    suspend fun setManagedIfaces(names: Collection<String>): Boolean = withContext(Dispatchers.IO) {
        val current = getConfig() ?: return@withContext false
        current.managedIfaces.clear()
        current.managedIfaces.addAll(names)
        setConfig(current)
    }

    /**
     * Long-lived subscription flow that receives real-time server-push JSON events
     */
    fun subscribeEvents(): Flow<JSONObject> = flow {
        var socket: LocalSocket? = null
        try {
            socket = connectSocket()
            val writer = BufferedWriter(OutputStreamWriter(socket.outputStream, Charsets.UTF_8))
            val reader = BufferedReader(InputStreamReader(socket.inputStream, Charsets.UTF_8))

            val subReq = JSONObject().apply {
                put("id", reqId.getAndIncrement())
                put("action", "subscribe")
            }
            writer.write(subReq.toString() + "\n")
            writer.flush()

            // Read subscription ack
            reader.readLine()

            // Continuous push event stream
            while (kotlin.coroutines.coroutineContext.isActive) {
                val line = reader.readLine() ?: break
                if (line.trim().isNotEmpty()) {
                    try {
                        val json = JSONObject(line)
                        if (json.has("event")) {
                            emit(json)
                        }
                    } catch (e: Exception) {
                        Log.w(TAG, "Invalid event JSON: $line")
                    }
                }
            }
        } catch (e: Exception) {
            Log.w(TAG, "Event subscription disconnected: ${e.message}")
        } finally {
            try { socket?.close() } catch (_: Exception) {}
        }
    }.flowOn(Dispatchers.IO)
}
