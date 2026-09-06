package com.vpnshareproxy.app.util

import java.net.Inet6Address
import java.net.InetAddress

/** Strict IP literal checks for DNS fields (reject hostnames). */
object IpFormats {
    private val IPV4 = Regex(
        """^(?:(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)\.){3}(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)$"""
    )

    fun isIpv4(raw: String): Boolean = IPV4.matches(raw.trim())

    fun isIpv6(raw: String): Boolean {
        val s = raw.trim()
        if (s.isEmpty() || !s.contains(':')) return false
        if (!s.all { it.isDigit() || it in 'a'..'f' || it in 'A'..'F' || it == ':' }) {
            return false
        }
        return try {
            InetAddress.getByName(s) is Inet6Address
        } catch (_: Exception) {
            false
        }
    }
}
