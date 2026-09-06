#!/system/bin/sh
# =============================================================================
# VPN Share Proxy Native Service Launcher
# =============================================================================
MODDIR="${0%/*}"

# Wait for system network tables to be initialized on boot
wait_count=0
while [ ! -f /data/misc/net/rt_tables ]; do
    sleep 2
    wait_count=$((wait_count + 1))
    [ $wait_count -ge 30 ] && break
done

chmod 755 "$MODDIR/bin/vspd" "$MODDIR/bin/vsp-ctl" 2>/dev/null || true
export MODDIR="$MODDIR"
nohup "$MODDIR/bin/vspd" > /data/local/tmp/vspd.log 2>&1 &
