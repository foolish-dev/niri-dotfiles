#!/bin/sh
awk '
/^MemTotal:/     { t = $2 }
/^MemAvailable:/ { a = $2 }
END {
  u = int((t - a) / t * 100)
  n = int(u / 12.5)
  bar = ""
  for (i = 0; i < 8; i++) bar = bar (i < n ? "█" : "░")
  printf "%s %d%%", bar, u
}' /proc/meminfo
