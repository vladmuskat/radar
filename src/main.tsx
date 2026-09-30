import React from 'react';
import ReactDOM from 'react-dom/client';
import { App } from './App';
import './styles.css';
import { installGlobalLogging, logFailure } from './logging';

const disposeLogging = installGlobalLogging();
import.meta.hot?.dispose(disposeLogging);

class ErrorBoundary extends React.Component<React.PropsWithChildren, { error: string | null }> {
  state: { error: string | null } = { error: null };
  /** Reports render failures without forwarding their arbitrary message text. */
  componentDidCatch(error: Error) {
    logFailure('react_render_failed', error);
  }
  /** Switches the subtree to a stable fallback after a render failure. */
  static getDerivedStateFromError(error: Error) {
    return { error: error.message };
  }
  /** Renders the fallback or the healthy application subtree. */
  render() {
    if (this.state.error)
      return (
        <div className="fatal">
          <h1>Не удалось отобразить интерфейс</h1>
          <p>{this.state.error}</p>
          <button onClick={() => location.reload()}>Перезапустить интерфейс</button>
        </div>
      );
    return this.props.children;
  }
}
ReactDOM.createRoot(document.getElementById('root')!).render(
  <ErrorBoundary>
    <App />
  </ErrorBoundary>,
);
