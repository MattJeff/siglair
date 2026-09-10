#!/usr/bin/env bash
# Le contrat du service `browser` de compose.yml, lu par la CI.
#
# Le port CDP n'a aucune authentification et Chromium sort sur Internet : ce
# qui protège la base, c'est qu'aucun port n'est publié et que `db` ne résout
# pas depuis une page. Une ligne de compose.yml retirée par mégarde ne se voit
# pas au déploiement — `up -d` réussit, l'API est saine — d'où ce script.
#
# PyYAML plutôt que grep : un `ports:` commenté ou indenté sous un autre
# service ne doit ni rassurer ni alarmer. Le runner ubuntu-latest l'embarque ;
# ailleurs, `pip3 install --user pyyaml`.
set -euo pipefail
cd "$(dirname "$0")/.."

python3 - compose.yml <<'EOF'
import sys, yaml

svc = yaml.safe_load(open(sys.argv[1]))["services"]
b = svc.get("browser") or sys.exit("browser: service absent")
errs = []

if "ports" in b:
    errs.append("browser publie un port : le port CDP n'a pas d'authentification")
if not b.get("mem_limit"):
    errs.append("browser sans mem_limit : un Chromium sans plafond emporte la base avec lui")
if "@sha256:" not in str(b.get("image", "")):
    errs.append("browser n'est pas épinglé par digest")

args = [str(a) for a in (b.get("entrypoint") or []) + (b.get("command") or [])]
rules = next((a for a in args if a.startswith("--host-resolver-rules=")), "")
for name in ("db", "api", "web", "caddy", "localhost"):
    if f"MAP {name} ~NOTFOUND" not in rules:
        errs.append(f"--host-resolver-rules ne bloque pas `{name}`")

dep = (svc.get("api", {}).get("depends_on") or {})
if not (isinstance(dep, dict) and dep.get("browser", {}).get("condition") == "service_healthy"):
    errs.append("api ne dépend pas de browser (condition: service_healthy)")

if errs:
    sys.exit("compose.yml :\n  - " + "\n  - ".join(errs))
print("compose.yml : le service browser respecte son contrat")
EOF
