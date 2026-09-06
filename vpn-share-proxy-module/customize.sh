#!/system/bin/sh
SKIPUNZIP=0

ui_print "- Installing VPN Share Proxy (native)..."

set_perm_recursive "$MODPATH/bin" 0 0 0755 0755
set_perm "$MODPATH/service.sh" 0 0 0755

if [ ! -f "$MODPATH/config.json" ]; then
    cat > "$MODPATH/config.json" <<'CFG'
{
  "enable_ipv4": true,
  "enable_ipv6": false,
  "dns_redirect": "off",
  "custom_dns_v4": null,
  "custom_dns_v6": null,
  "proxy_mode": "global",
  "mac_allow_list": [],
  "mac_block_list": []
}
CFG
fi

ui_print "- Installation successful!"
ui_print "- Reboot device to activate."
