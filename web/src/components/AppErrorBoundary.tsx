import { Component } from 'react';
import type { ErrorInfo, ReactNode } from 'react';
import { Button } from './Button';

interface Props {
  children: ReactNode;
}

interface State {
  failed: boolean;
}

export class AppErrorBoundary extends Component<Props, State> {
  override state: State = { failed: false };

  static getDerivedStateFromError(): State {
    return { failed: true };
  }

  override componentDidCatch(error: Error, info: ErrorInfo) {
    console.error('Erreur React non récupérée', error, info.componentStack);
  }

  override render() {
    if (!this.state.failed) return this.props.children;

    return (
      <main
        style={{
          display: 'grid',
          minHeight: '70dvh',
          placeItems: 'center',
          padding: 24,
          textAlign: 'center',
        }}
      >
        <div style={{ maxWidth: 520 }}>
          <h1>L’application doit être resynchronisée</h1>
          <p style={{ margin: '14px 0 20px', color: 'var(--muted)' }}>
            Une nouvelle version a peut-être été déployée pendant que cet onglet était ouvert.
            Rechargez la page pour reprendre là où vous en étiez.
          </p>
          <Button onClick={() => location.reload()}>Recharger l’application</Button>
        </div>
      </main>
    );
  }
}
