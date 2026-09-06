# VPN Share Proxy Daemon (vspd) - Unix Domain Socket IPC 协议规范

本文档面向 **Android 客户端应用 (App)、WebUI 及 CLI 开发者**，定义了通过 Unix 域套接字 (UDS) 与 `vspd` 守护进程通信的协议标准。

---

## 1. 传输层规范 (Transport Layer)

- **套接字类型**：Unix Domain Stream Socket (`AF_UNIX`, `SOCK_STREAM`)。
- **套接字路径**：
  - 默认文件系统路径：`/dev/socket/vpn-share-proxy.sock` 或 `/data/local/tmp/vpn-share-proxy.sock`。
  - 抽象命名空间（Abstract Namespace，避免权限与遗留文件问题）：`@vpn-share-proxy.sock`。
- **协议帧格式**：**换行符分隔的 JSON (Newline Delimited JSON, NDJSON)**。
  - 每个请求（Request）或响应（Response）均为合法的单行 JSON 字符串，以 `\n` 结尾。
  - 编码必须为 `UTF-8`。

---

## 2. 交互模式 (Interaction Modes)

本协议支持两种交互模式：
1. **RPC 模式 (Request-Response)**：客户端发送请求对象（携带 `id`），服务端处理完成后返回携带相同 `id` 的响应对象。
2. **事件流订阅模式 (Pub/Sub Event Streaming)**：客户端发起订阅后保持长连接，服务端在特定事件发生时（如 VPN 掉线、新设备连入）主动向客户端单向推送事件消息（不含 `id` 字段）。

---

## 3. RPC 消息定义

### 3.1 通用请求格式 (Request)
```json
{
  "id": 1001,
  "action": "<ACTION_NAME>",
  "params": { ... }
}
```
| 字段 | 类型 | 说明 |
| :--- | :--- | :--- |
| `id` | 整型 / 字符串 | 客户端请求的唯一标识符，服务端原样回传 |
| `action` | 字符串 | 调用的方法名 |
| `params` | 对象 (可选) | 请求参数 |

### 3.2 通用响应格式 (Response)
```json
{
  "id": 1001,
  "code": 0,
  "msg": "OK",
  "data": { ... }
}
```
| 字段 | 类型 | 说明 |
| :--- | :--- | :--- |
| `id` | 整型 / 字符串 | 对应客户端请求的 `id` |
| `code` | 整型 | 状态码，`0` 表示成功，非 0 表示错误 |
| `msg` | 字符串 | 提示信息或错误原因 |
| `data` | 任意 (可选) | 业务返回数据 |

---

## 4. 核心 RPC 接口列表

### 4.1 获取服务整体状态 (`get_status`)
- **请求**:
  ```json
  {"id": 1, "action": "get_status"}
  ```
- **响应**:
  ```json
  {
    "id": 1,
    "code": 0,
    "msg": "OK",
    "data": {
      "state": "Active", // "Active" | "WaitingVpn" | "Standby"
      "tun_interface": "tun0", // 当前检测到的 VPN 接口，未检测到为 null
      "table_id": 1038, // Android 绑定的 VPN 路由表 ID
      "hotspot_interfaces": ["ap0", "wlan0"],
      "ipv4_active": true,
      "ipv6_active": false,
      "client_count": 3,
      "uptime_secs": 18234,
      "version": "0.1.0"
    }
  }
  ```

### 4.2 获取当前配置 (`get_config`)
- **请求**:
  ```json
  {"id": 2, "action": "get_config"}
  ```
- **响应**:
  ```json
  {
    "id": 2,
    "code": 0,
    "msg": "OK",
    "data": {
      "enable_ipv4": true,
      "enable_ipv6": false,
      "dns_redirect": "auto", // "auto" | "custom" | "off"
      "custom_dns_v4": "1.1.1.1",
      "custom_dns_v6": "2606:4700:4700::1111",
      "proxy_mode": "global", // "global" | "mac_allowlist" | "mac_blocklist"
      "mac_allow_list": ["AA:BB:CC:11:22:33"],
      "mac_block_list": []
    }
  }
  ```

### 4.3 动态修改配置 (`set_config`)
修改配置后，守护进程会在毫秒级内自动触发调和引擎应用新规则，无需重启。
- **请求**:
  ```json
  {
    "id": 3,
    "action": "set_config",
    "params": {
      "enable_ipv4": true,
      "enable_ipv6": true,
      "dns_redirect": "custom",
      "custom_dns_v4": "8.8.8.8",
      "proxy_mode": "mac_allowlist",
      "mac_allow_list": ["AA:BB:CC:11:22:33", "12:34:56:78:9A:BC"]
    }
  }
  ```
- **响应**:
  ```json
  {
    "id": 3,
    "code": 0,
    "msg": "Configuration saved and applied"
  }
  ```

### 4.4 获取热点连接设备列表 (`list_devices`)
- **请求**:
  ```json
  {"id": 4, "action": "list_devices"}
  ```
- **响应**:
  ```json
  {
    "id": 4,
    "code": 0,
    "msg": "OK",
    "data": [
      {
        "ip": "192.168.43.101",
        "mac": "aa:bb:cc:11:22:33",
        "iface": "ap0",
        "hostname": "iPad-Pro",
        "policy": "allowed", // "allowed" | "blocked" | "default"
        "first_seen": 1725368000,
        "last_seen": 1725368200
      },
      {
        "ip": "192.168.43.105",
        "mac": "54:e4:3a:88:99:00",
        "iface": "ap0",
        "hostname": "Windows-PC",
        "policy": "default",
        "first_seen": 1725368100,
        "last_seen": 1725368210
      }
    ]
  }
  ```

### 4.5 快捷切换设备黑白名单策略 (`set_device_policy`)
- **请求**:
  ```json
  {
    "id": 5,
    "action": "set_device_policy",
    "params": {
      "mac": "54:e4:3a:88:99:00",
      "policy": "block" // "block" | "allow" | "default"
    }
  }
  ```
- **响应**:
  ```json
  {
    "id": 5,
    "code": 0,
    "msg": "Device policy updated"
  }
  ```

### 4.6 强制立即执行调和 (`reconcile_now`)
- **请求**:
  ```json
  {"id": 6, "action": "reconcile_now"}
  ```
- **响应**:
  ```json
  {
    "id": 6,
    "code": 0,
    "msg": "Reconciliation completed",
    "data": {
      "rules_applied": true,
      "tun": "tun0",
      "table": 1038
    }
  }
  ```

---

## 5. 事件订阅与实时下发 (Pub/Sub Event Streaming)

### 5.1 发起订阅 (`subscribe`)
客户端在同一连接上发送此指令后，该连接进入事件监听模式：
```json
{
  "id": 10,
  "action": "subscribe",
  "params": {
    "topics": ["vpn", "device", "state", "reconcile"]
  }
}
```
响应确认：
```json
{
  "id": 10,
  "code": 0,
  "msg": "Subscribed to topics: [vpn, device, state, reconcile]"
}
```

### 5.2 服务端推送的事件帧 (Event Frames)
推送的事件**不包含 `id` 字段**，通过 `event` 字段区分：

#### (1) VPN 状态变更事件 (`vpn_changed`)
```json
{
  "event": "vpn_changed",
  "timestamp": 1725368250,
  "data": {
    "status": "up", // "up" | "down"
    "iface": "tun0",
    "table_id": 1038
  }
}
```

#### (2) 热点设备连接事件 (`device_connected`)
```json
{
  "event": "device_connected",
  "timestamp": 1725368260,
  "data": {
    "ip": "192.168.43.108",
    "mac": "1a:2b:3c:4d:5e:6f",
    "iface": "ap0"
  }
}
```

#### (3) 热点设备离线事件 (`device_disconnected`)
```json
{
  "event": "device_disconnected",
  "timestamp": 1725368300,
  "data": {
    "mac": "1a:2b:3c:4d:5e:6f"
  }
}
```

#### (4) 规则调和完成事件 (`reconcile_finished`)
```json
{
  "event": "reconcile_finished",
  "timestamp": 1725368301,
  "data": {
    "success": true,
    "active_tun": "tun0",
    "rules_count": 7
  }
}
```

---

## 6. Android App 端接入示例 (Kotlin 协程伪代码)

```kotlin
class VpnGatewayClient(private val socketPath: String = "/data/local/tmp/vpn-share-proxy.sock") {
    private var socket: LocalSocket? = null
    private var writer: BufferedWriter? = null
    private var reader: BufferedReader? = null

    suspend fun connectAndSubscribe(onEvent: (String, JSONObject) -> Unit) = withContext(Dispatchers.IO) {
        val address = LocalSocketAddress(socketPath, LocalSocketAddress.Namespace.FILESYSTEM)
        socket = LocalSocket().apply { connect(address) }
        writer = socket!!.outputStream.bufferedWriter()
        reader = socket!!.inputStream.bufferedReader()

        // 发送订阅请求
        writer?.write("""{"id": 1, "action": "subscribe", "params": {"topics": ["all"]}}""" + "\n")
        writer?.flush()

        // 持续读取推送事件
        while (isActive) {
            val line = reader?.readLine() ?: break
            val json = JSONObject(line)
            if (json.has("event")) {
                val eventType = json.getString("event")
                val data = json.getJSONObject("data")
                withContext(Dispatchers.Main) {
                    onEvent(eventType, data)
                }
            }
        }
    }
}
```
