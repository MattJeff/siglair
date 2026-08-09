/**
 * FAQ en <details>/<summary> natifs : ouvrable au clavier, annoncée par les
 * lecteurs d'écran, zéro JavaScript (docs/DESIGN.md §7 du portage).
 *
 * Le balisage JSON-LD est généré DEPUIS le même tableau que l'affichage : les
 * deux ne peuvent plus diverger, et Google sanctionne le contraire.
 */
import { Link } from 'react-router-dom';
import s from './faq.module.css';

export interface QA {
  q: string;
  /** Texte simple : c'est lui qui part aussi dans le JSON-LD. */
  a: string;
  /** Lien « pour aller plus loin », hors réponse. */
  more?: { to: string; label: string };
}

function jsonLd(items: QA[]): string {
  const data = {
    '@context': 'https://schema.org',
    '@type': 'FAQPage',
    mainEntity: items.map((it) => ({
      '@type': 'Question',
      name: it.q,
      acceptedAnswer: { '@type': 'Answer', text: it.a },
    })),
  };
  // Un "</script>" dans une réponse fermerait la balise : on neutralise le <.
  return JSON.stringify(data).replace(/</g, '\\u003c');
}

export function Faq({ items }: { items: QA[] }) {
  return (
    <div className={s.list}>
      {items.map((it) => (
        <details className={s.item} key={it.q}>
          <summary className={s.q}>{it.q}</summary>
          <div className={s.a}>
            <p>{it.a}</p>
            {it.more && <p><Link to={it.more.to}>{it.more.label}</Link></p>}
          </div>
        </details>
      ))}
      <script type="application/ld+json" dangerouslySetInnerHTML={{ __html: jsonLd(items) }} />
    </div>
  );
}
