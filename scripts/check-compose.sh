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

# --- docker/Caddyfile : les seuls chemins d'API publiés ----------------------
#
# Même angle mort que ci-dessus, un cran plus haut. Le Caddyfile n'est pas dans
# l'image : le job `deploy` ne le copie pas, il se pousse à la main (§4 de
# docs/DEPLOY_SIGLAIR.md). Une ligne perdue ne se voit donc ni à la CI ni au
# `up -d` — seulement à la première requête d'un client qui n'était pas là pour
# tester. Deux erreurs opposées à attraper, et pas une de plus :
#
#   - `/v1/mcp/server` retiré du matcher : le serveur MCP part sur la console
#     Next.js, qui répond 404, et le Claude Code du fondateur n'a plus rien ;
#   - un joker `/v1/mcp/*` ajouté « pour faire simple » : il publie du même coup
#     `/v1/mcp/catalog`, `/v1/mcp/connect`, `/v1/mcp/servers` et
#     `/v1/mcp/oauth/start`, le catalogue d'intégrations réservé à la console.
#
# grep et pas un parseur : `caddy validate` dirait la syntaxe, pas la politique,
# et il faudrait l'image pour l'exécuter. En contrepartie ce contrôle ne connaît
# que la forme sur une ligne, `@api path …` ; un matcher réécrit en bloc est un
# échec, exprès — mieux vaut relire que croire un contrôle devenu aveugle.
matcher=$(grep -E '^[[:space:]]*@api path ' docker/Caddyfile) || {
	echo "docker/Caddyfile : aucune ligne \`@api path …\`. Matcher réécrit ? Relire à la main." >&2
	exit 1
}

caddy_errs=()
for p in $matcher; do
	case "$p" in
	/v1/mcp*\**) caddy_errs+=("le matcher @api contient le joker \`$p\` : il publierait le catalogue d'intégrations (/v1/mcp/catalog, /connect, /servers, /oauth/start), réservé à la console") ;;
	esac
done
case " $matcher " in
*" /v1/mcp/server "*) ;;
*) caddy_errs+=("/v1/mcp/server absent du matcher @api : le serveur MCP part sur la console et répond 404") ;;
esac

if ((${#caddy_errs[@]})); then
	printf 'docker/Caddyfile :\n' >&2
	printf '  - %s\n' "${caddy_errs[@]}" >&2
	exit 1
fi
echo "docker/Caddyfile : /v1/mcp/server est publié, et sans joker sous /v1/mcp"
