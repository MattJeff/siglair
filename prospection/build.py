#!/usr/bin/env python3
"""Filtre la liste publique des OF (DGEFP) -> prospection Qualiopi >= 3 formateurs.

Entree : listeOF.csv (CSV ';' UTF-8, telecharge sur monactiviteformation.emploi.gouv.fr)
Sortie : prospection/organismes-qualiopi.csv
"""
import csv, sys

SRC = sys.argv[1]
DST = sys.argv[2]

# ponytail: codes region INSEE en dur, 18 valeurs qui ne bougent pas. Code inconnu -> code brut.
REGIONS = {
    "01": "Guadeloupe", "02": "Martinique", "03": "Guyane", "04": "La Reunion",
    "06": "Mayotte", "11": "Ile-de-France", "24": "Centre-Val de Loire",
    "27": "Bourgogne-Franche-Comte", "28": "Normandie", "32": "Hauts-de-France",
    "44": "Grand Est", "52": "Pays de la Loire", "53": "Bretagne",
    "75": "Nouvelle-Aquitaine", "76": "Occitanie", "84": "Auvergne-Rhone-Alpes",
    "93": "Provence-Alpes-Cote d'Azur", "94": "Corse",
}
CERT_COLS = ["certifications.actionsDeFormation", "certifications.bilansDeCompetences",
             "certifications.VAE", "certifications.actionsDeFormationParApprentissage"]
PERIMETRES = ["Formation", "Bilan de competences", "VAE", "Apprentissage"]


def to_int(v):
    v = v.strip()
    return int(v) if v.isdigit() else None


def main():
    rows, total, certifies, malformees = [], 0, 0, 0
    with open(SRC, encoding="utf-8", newline="") as f:
        r = csv.reader(f, delimiter=";")
        hdr = next(r)
        ix = {h: i for i, h in enumerate(hdr)}
        for row in r:
            total += 1
            if len(row) != len(hdr):
                malformees += 1
                continue
            perims = [p for p, c in zip(PERIMETRES, CERT_COLS)
                      if row[ix[c]].strip().lower() == "true"]
            if not perims:
                continue
            certifies += 1
            eff = to_int(row[ix["informationsDeclarees.effectifFormateurs"]])
            if eff is None or eff < 3:
                continue
            rows.append([
                row[ix["denomination"]].strip(),
                row[ix["siren"]].strip(),
                row[ix["siretEtablissementDeclarant"]].strip(),
                REGIONS.get(row[ix["adressePhysiqueOrganismeFormation.codeRegion"]].strip(),
                            row[ix["adressePhysiqueOrganismeFormation.codeRegion"]].strip()),
                row[ix["adressePhysiqueOrganismeFormation.ville"]].strip(),
                row[ix["adressePhysiqueOrganismeFormation.codePostal"]].strip(),
                row[ix["informationsDeclarees.specialitesDeFormation.libelleSpecialite1"]].strip(),
                eff,
                to_int(row[ix["informationsDeclarees.nbStagiaires"]]) or 0,
                "|".join(perims),
                row[ix["numeroDeclarationActivite"]].strip(),
                row[ix["informationsDeclarees.dateDerniereDeclaration"]].strip(),
            ])

    rows.sort(key=lambda x: (-x[7], -x[8]))
    with open(DST, "w", encoding="utf-8", newline="") as f:
        w = csv.writer(f)
        w.writerow(["raison_sociale", "siren", "siret", "region", "ville", "code_postal",
                    "specialite_principale", "effectif_formateurs", "nb_stagiaires",
                    "perimetre_certification", "num_declaration_activite", "date_derniere_declaration"])
        w.writerows(rows)

    print(f"lignes_entree={total} malformees={malformees} certifies={certifies} retenues={len(rows)}")
    return rows


def check():
    """ponytail: un seul garde-fou, sur la logique qui compte (filtre + tri)."""
    rows = main()
    assert rows, "sortie vide"
    assert all(r[7] >= 3 for r in rows), "effectif < 3 dans la sortie"
    assert all(rows[i][7] >= rows[i + 1][7] for i in range(len(rows) - 1)), "tri non decroissant"
    assert all(r[1].isdigit() and len(r[1]) == 9 for r in rows if r[1]), "SIREN invalide"
    print("check OK")


if __name__ == "__main__":
    check()
