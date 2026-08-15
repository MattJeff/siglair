#!/usr/bin/env bash
#
# Siglair — parcours de bout en bout. Le juge du projet.
#
#   « ça compile » ne veut rien dire.
#   « le GIF fait 240 Ko et le clic est compté » veut dire quelque chose.
#
# Usage :
#   docker compose up -d --build
#   scripts/smoke.sh
#
# Variables :
#   API=http://localhost:8080     racine de l'API à tester
#   SQL="psql postgres://..."     pour interroger une base hors docker compose
#
set -euo pipefail

cd "$(dirname "$0")/.."

API="${API:-http://localhost:8080}"
WAIT_API="${WAIT_API:-90}"
WAIT_RENDER="${WAIT_RENDER:-180}"
TMP="$(mktemp -d)"

# Le compte de test ne survit JAMAIS à ce script, même quand il meurt en cours de route.
#
# L'étape 7 le supprime par la vraie route RGPD — mais elle est la dernière, et tout échec
# avant elle laissait derrière un compte, une organisation et des événements. Ça s'est vu :
# le déploiement interrompu du 13 août a laissé un compte fantôme en production, qui fausse
# la seule métrique qu'on regarde vraiment au démarrage — combien d'inscrits.
#
# Le filet passe par SQL et non par l'API : quand le script échoue tôt, il n'y a souvent pas
# encore de session utilisable. `|| true` partout — un filet de sécurité qui fait échouer le
# déploiement parce que le ménage a raté serait pire que le désordre qu'il nettoie.
nettoyer() {
    local code=$?
    if [[ -n "${EMAIL:-}" ]]; then
        sql "DELETE FROM orgs WHERE id IN (
               SELECT m.org_id FROM org_members m
               JOIN users u ON u.id = m.user_id WHERE u.email = '$EMAIL');" >/dev/null 2>&1 || true
        sql "DELETE FROM users       WHERE email = '$EMAIL';" >/dev/null 2>&1 || true
        sql "DELETE FROM magic_links WHERE email = '$EMAIL';" >/dev/null 2>&1 || true
    fi
    rm -rf "$TMP"
    exit "$code"
}
trap nettoyer EXIT

STEP=0
step() { STEP=$((STEP + 1)); printf '\n\033[1m[%02d] %s\033[0m\n' "$STEP" "$1"; }
ok()   { printf '     \033[32mok\033[0m   %s\n' "$1"; }
info() { printf '     ..   %s\n' "$1"; }
die()  { printf '\n\033[31mÉCHEC — %s\033[0m\n\n' "$1" >&2; exit 1; }

for bin in curl jq openssl; do
    command -v "$bin" >/dev/null || die "$bin est requis — Debian/Ubuntu : apt install $bin · macOS : brew install $bin"
done

# Requêtes SQL : par défaut on passe par le conteneur, aucune installation locale.
if [[ -n "${SQL:-}" ]]; then
    sql() { $SQL -At -c "$1"; }
else
    command -v docker >/dev/null || die "docker est requis, ou définissez SQL=\"psql <url>\""
    sql() { docker compose exec -T db psql -U siglair -d siglair -At -c "$1"; }
fi

# ---------------------------------------------------------------- 1. liveness

step "L'API répond"
deadline=$((SECONDS + WAIT_API))
until curl -fsS "$API/health" -o "$TMP/health" 2>/dev/null; do
    ((SECONDS < deadline)) || die "/health muet après ${WAIT_API}s — 'docker compose logs api'"
    sleep 1
done
jq -e '.ok == true' "$TMP/health" >/dev/null || die "/health ne renvoie pas {\"ok\":true}"
ok "/health"

curl -fsS "$API/ready" -o /dev/null || die "/ready en échec : la base ne répond pas"
ok "/ready — base joignable"

curl -fsS "$API/api/config" -o "$TMP/config" || die "/api/config ne répond pas"
jq -e 'has("google") and has("apple") and has("magic") and has("billing")' "$TMP/config" >/dev/null \
    || die "/api/config incomplet : $(cat "$TMP/config")"
ok "/api/config — $(jq -c . "$TMP/config")"

# ---------------------------------------------------------------- 2. connexion

EMAIL="smoke-$(date +%s)-$$@siglair.test"

step "Lien magique"
code=$(curl -sS -o "$TMP/magic" -w '%{http_code}' \
    -X POST "$API/api/auth/magic/request" \
    -H 'Content-Type: application/json' \
    -d "{\"email\":\"$EMAIL\"}")
case "$code" in
    204) ok "demande acceptée (204)" ;;
    # 5 demandes/heure/IP (contrat §5.2) : au-delà, le test reste valide, il
    # injecte son propre jeton juste après.
    429) info "limite de débit atteinte (429) — attendu si le script tourne en boucle" ;;
    *)   die "POST /api/auth/magic/request → $code : $(cat "$TMP/magic")" ;;
esac

# On n'envoie pas d'e-mail en test : on insère un jeton dont on connaît la valeur
# en clair, haché comme le fait util::hash_token (sha256 brut).
TOKEN="$(openssl rand -hex 24)"
sql "INSERT INTO magic_links (id, email, token_hash, expires_at)
     VALUES (gen_random_uuid(), '$EMAIL', sha256(convert_to('$TOKEN','UTF8')), now() + interval '15 minutes');" \
    >/dev/null || die "insertion du jeton impossible — la base est-elle migrée ?"
ok "jeton injecté pour $EMAIL"

code=$(curl -sS -o /dev/null -D "$TMP/hdr" -w '%{http_code}' "$API/api/auth/magic/consume?token=$TOKEN")
[[ "$code" == 30[123] ]] || die "consume → $code, attendu une redirection vers /app"

COOKIE=$(grep -i '^set-cookie:' "$TMP/hdr" | grep -i 'sig_session' | head -1 \
    | sed -E 's/^[Ss]et-[Cc]ookie: *([^;]*).*/\1/' | tr -d '\r')
[[ -n "$COOKIE" ]] || die "consume n'a posé aucun cookie de session"
ok "session ouverte (${COOKIE%%=*})"

curl -fsS -H "Cookie: $COOKIE" "$API/api/me" -o "$TMP/me" || die "/api/me refuse la session"
ok "/api/me — plan $(jq -r '.plan // "?"' "$TMP/me")"

# Le GIF hébergé est réservé aux plans payants (contrat §6). Pas de Stripe dans
# un smoke test : on bascule l'org en pro directement en base.
sql "UPDATE orgs SET plan = 'pro' WHERE id IN (
       SELECT m.org_id FROM org_members m
       JOIN users u ON u.id = m.user_id WHERE u.email = '$EMAIL');" >/dev/null
ok "org basculée en plan pro"

# ---------------------------------------------------------------- 3. signature

step "Création de la signature"
cat > "$TMP/doc.json" <<'JSON'
{
  "name": "Smoke",
  "doc": {
    "v": 1,
    "canvas": { "width": 480, "height": 160, "bg": "#07111f", "bgImage": "", "overlay": 0.8, "radius": 18 },
    "timelineDuration": 2.0,
    "elements": [
      { "id": "nam0001", "type": "text", "x": 24, "y": 24, "w": 380, "h": 30,
        "content": "{{name}} — {{role}}", "fontSize": 20, "fontWeight": "700", "color": "#ffffff",
        "anim": { "preset": "fade", "duration": 1.2, "iterations": "1" } },
      { "id": "cta0001", "type": "button", "x": 24, "y": 84, "w": 180, "h": 40,
        "content": "Voir le site", "href": "https://example.com/",
        "background": "#2563eb", "color": "#ffffff", "radius": 10, "align": "center",
        "anim": { "preset": "pulse", "duration": 1.5, "iterations": "infinite" } }
    ]
  }
}
JSON
code=$(curl -sS -o "$TMP/sig" -w '%{http_code}' -X POST "$API/api/signatures" \
    -H "Cookie: $COOKIE" -H 'Content-Type: application/json' --data-binary @"$TMP/doc.json")
[[ "$code" == "201" ]] || die "POST /api/signatures → $code : $(cat "$TMP/sig")"
SIG_ID=$(jq -r '.id // .signature.id' "$TMP/sig")
[[ "$SIG_ID" != "null" && -n "$SIG_ID" ]] || die "pas d'id dans la réponse : $(cat "$TMP/sig")"
ok "signature $SIG_ID"

curl -fsS -o /dev/null -X PATCH "$API/api/signatures/$SIG_ID" \
    -H "Cookie: $COOKIE" -H 'Content-Type: application/json' \
    -d '{"profile":{"name":"Ada Lovelace","role":"Fondatrice","website":"https://example.com/"}}' \
    || die "PATCH du profil refusé"
ok "profil renseigné"

curl -fsS -H "Cookie: $COOKIE" "$API/api/signatures/$SIG_ID/export?mode=hosted" -o "$TMP/export" \
    || die "export hosted refusé"
grep -q '{{' <<<"$(jq -r .html "$TMP/export")" && die "des jetons {{...}} non résolus fuient dans l'export"
ok "export hosted — jetons résolus"

# ---------------------------------------------------------------- 4. rendu

step "Publication et rendu"
code=$(curl -sS -o "$TMP/pub" -w '%{http_code}' -X POST "$API/api/signatures/$SIG_ID/publish" -H "Cookie: $COOKIE")
[[ "$code" == "200" || "$code" == "201" ]] || die "publish → $code : $(cat "$TMP/pub")"
SLUG=$(jq -r '.slug' "$TMP/pub")
[[ -n "$SLUG" && "$SLUG" != "null" ]] || die "pas de slug renvoyé : $(cat "$TMP/pub")"
ok "slug $SLUG"

deadline=$((SECONDS + WAIT_RENDER))
while :; do
    curl -fsS -H "Cookie: $COOKIE" "$API/api/signatures/$SIG_ID/status" -o "$TMP/status"
    status=$(jq -r '.job.status // "?"' "$TMP/status")
    case "$status" in
        done) break ;;
        failed) die "rendu en échec : $(jq -r '.job.error // "sans message"' "$TMP/status")" ;;
    esac
    ((SECONDS < deadline)) || die "rendu toujours '$status' après ${WAIT_RENDER}s — 'docker compose logs renderer'"
    sleep 2
done
ok "job terminé en $(jq -r '.render.frames // "?"' "$TMP/status") frames"

# ---------------------------------------------------------------- 5. le produit

step "GIF public"
code=$(curl -sS -o "$TMP/out.gif" -w '%{http_code}' "$API/s/$SLUG.gif")
[[ "$code" == "200" ]] || die "GET /s/$SLUG.gif → $code"

magic=$(head -c 6 "$TMP/out.gif")
[[ "$magic" == "GIF89a" ]] || die "les 6 premiers octets sont '$magic', pas 'GIF89a' — ce n'est pas un GIF animé"

bytes=$(wc -c < "$TMP/out.gif" | tr -d ' ')
((bytes > 2000))    || die "GIF de $bytes octets : bien trop petit pour 2 secondes d'animation"
((bytes < 2000000)) || die "GIF de $bytes octets : au-delà de 2 Mo, Gmail le coupe (contrat §7.7)"
ok "GIF89a, $((bytes / 1024)) Ko"

code=$(curl -sS -o "$TMP/out.png" -w '%{http_code}' "$API/s/$SLUG.png")
[[ "$code" == "200" ]] || die "GET /s/$SLUG.png → $code (repli Outlook legacy)"
[[ "$(head -c 4 "$TMP/out.png" | tail -c 3)" == "PNG" ]] || die "/s/$SLUG.png ne renvoie pas un PNG"
ok "PNG de repli, $(( $(wc -c < "$TMP/out.png") / 1024 )) Ko"

opens=$(sql "SELECT count(*) FROM events WHERE signature_id = '$SIG_ID' AND kind = 'open';")
((opens >= 2)) || die "$opens ouverture(s) en base, 2 attendues (gif + png)"
ok "$opens ouvertures enregistrées"

step "Clic tracé"

# Le scanner passe EN PREMIER, et l'ordre n'est pas un détail.
#
# Microsoft Defender Safe Links, Proofpoint et Mimecast ouvrent chaque lien d'un e-mail
# avant de le remettre au destinataire. Ils doivent être redirigés comme tout le monde — un
# scanner qui reçoit une erreur classe le lien comme suspect, et c'est la signature entière
# qui devient douteuse — mais jamais comptés.
#
# En premier parce que la déduplication des clics porte sur (signature, élément, ip_hash) :
# si le clic humain avait lieu avant, celui du scanner serait écarté par la déduplication et
# le test passerait pour la mauvaise raison, sans rien prouver du filtre.
SCANNER='Mozilla/5.0 (compatible; MSIE 9.0; Windows NT 6.1) SafeLinks'
code=$(curl -sS -o /dev/null -w '%{http_code}' -A "$SCANNER" "$API/c/$SLUG/cta0001")
[[ "$code" == "302" ]] || die "un scanner d'URL doit être redirigé comme tout le monde, reçu $code"
robot=$(sql "SELECT count(*) FROM events WHERE signature_id = '$SIG_ID' AND kind = 'click';")
((robot == 0)) || die "$robot clic(s) de scanner compté(s) : le filtre anti-robots ne mord pas"
ok "scanner d'URL redirigé mais non compté"

# `curl` par défaut s'annonce « curl/8.x » et se fait — à juste titre — écarter par le
# filtre. Le test simule un humain : il doit donc se présenter comme un navigateur.
HUMAIN='Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15'
code=$(curl -sS -o /dev/null -D "$TMP/click" -w '%{http_code}' -A "$HUMAIN" "$API/c/$SLUG/cta0001")
[[ "$code" == "302" ]] || die "clic → $code, attendu 302"
location=$(grep -i '^location:' "$TMP/click" | sed -E 's/^[Ll]ocation: *//' | tr -d '\r')
[[ "$location" == "https://example.com/" ]] || die "redirection vers '$location' au lieu de la cible du document"
ok "302 → $location"

clicks=$(sql "SELECT count(*) FROM events WHERE signature_id = '$SIG_ID' AND kind = 'click' AND element_id = 'cta0001';")
((clicks == 1)) || die "$clicks clic(s) en base, 1 attendu"
ok "clic compté"

ip_leak=$(sql "SELECT count(*) FROM events WHERE signature_id = '$SIG_ID' AND (octet_length(ip_hash) <> 16 OR ua_family NOT IN ('gmail','outlook','apple-mail','other'));")
((ip_leak == 0)) || die "$ip_leak événement(s) hors contrat §4.1 (ip_hash 16 octets, ua_family fermée)"
ok "vie privée — ip_hash 16 octets, ua_family normalisée"

# ------------------------------------------------- 7. suppression du compte (RGPD)

# Ce script tourne à CHAQUE déploiement. Sans cette étape, il laisse derrière lui un
# compte, une organisation, une signature, un rendu et des événements — à chaque fois.
# Au bout d'un an, le nombre d'utilisateurs affiché est faux et le disque se remplit de
# GIF fantômes.
#
# Le ménage passe par la VRAIE route de suppression plutôt que par un DELETE en base :
# ainsi il nettoie ET il teste le chemin RGPD du contrat §4.1, qui est autrement le
# seul parcours critique que personne n'exerce jamais avant qu'un utilisateur le demande.
step "Suppression du compte"

code=$(curl -sS -o "$TMP/del" -w '%{http_code}' -X DELETE \
       -H "Cookie: $COOKIE" "$API/api/me")
[[ "$code" == "204" || "$code" == "200" ]] || die "DELETE /api/me → $code : $(cat "$TMP/del")"
ok "compte supprimé ($code)"

# La cascade doit avoir tout emporté. Une signature orpheline, c'est une donnée
# personnelle conservée après une demande de suppression.
left=$(sql "SELECT (SELECT count(*) FROM users WHERE email = '$EMAIL')
                 + (SELECT count(*) FROM signatures WHERE id = '$SIG_ID')
                 + (SELECT count(*) FROM events WHERE signature_id = '$SIG_ID');")
((left == 0)) || die "$left ligne(s) survivent à la suppression du compte (contrat §4.1)"
ok "purge vérifiée — compte, signature et événements"

# La session doit être morte immédiatement, pas à l'expiration du cookie.
code=$(curl -sS -o /dev/null -w '%{http_code}' -H "Cookie: $COOKIE" "$API/api/me")
[[ "$code" == "401" ]] || die "la session survit à la suppression du compte (HTTP $code)"
ok "session révoquée"

# Le GIF publié ne doit plus être servi : il vivait dans un email déjà envoyé, mais
# c'est le prix d'une suppression de compte, et l'utilisateur en est averti.
code=$(curl -sS -o /dev/null -w '%{http_code}' "$API/s/$SLUG.gif")
[[ "$code" == "404" || "$code" == "402" || "$code" == "410" ]] \
    || die "le GIF est encore servi après suppression du compte (HTTP $code)"
ok "GIF public retiré ($code)"

# ---------------------------------------------------------------- résumé

printf '
\033[32m══════════════════════════════════════════════════\033[0m
 Parcours complet réussi.

   compte        %s  (supprimé, rien ne reste)
   signature     %s
   URL publique  %s/s/%s.gif
   GIF           %s Ko  (%s octets)
   événements    %s ouvertures, %s clic
\033[32m══════════════════════════════════════════════════\033[0m

' "$EMAIL" "$SIG_ID" "$API" "$SLUG" "$((bytes / 1024))" "$bytes" "$opens" "$clicks"
