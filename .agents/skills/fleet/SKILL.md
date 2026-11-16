---
name: fleet
description: Run a command, deploy or check on the 3 VPS (ssh <fr1> = fr1 builds (Chromium, LLVM), CI, training, heavy agents, ssh <fr2> = fr2 tests/lint/bench/research without limits plus edge proxy/CDN, ssh prd = usa production). Use for any remote ssh/scp/systemctl/build; never a heavy build on the PC.
---

1. Always bound it: `timeout 60 ssh <<fr1>|<fr2>|prd> '<cmd>'` (120 for scp/installs). Builds: only on `<fr1>`.
2. Exit 255 / slow host: `timeout 30 ssh <fr1> 'pkill -f gitleaks; pkill -u ubuntu -f user-data-dir=/tmp/lighthouse; uptime'` then retry.
3. Deploy a file: `timeout 120 ssh <fr1> 'sudo mkdir -p /srv/X && sudo chown -R ubuntu:ubuntu /srv/X' && scp -q <src> <fr1>:/srv/X/`.
4. Service: `timeout 60 ssh prd 'sudo systemctl restart <svc> && systemctl is-active <svc>'`.
5. Prove a deploy with content, not a status: the rendered page (screenshot), or a real API call whose response body is checked (AGENTS.md section 3b); a bare HTTP code proves nothing.
6. Never break prod: test on vps or a secondary port before `prd`. Detailed roles: memory fleet-roles.md.
