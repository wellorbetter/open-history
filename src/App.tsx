import { useEffect, useState } from 'react';
import { CompactView } from './routes/CompactView';
import { FullHistoryView } from './routes/FullHistoryView';
import { PreviewGallery } from './routes/PreviewGallery';
import { SetupView } from './routes/SetupView';

type Surface = 'compact' | 'history' | 'setup' | 'preview';

function readSurface(): Surface {
  const value = new URLSearchParams(window.location.search).get('surface');
  return value === 'history' || value === 'setup' || value === 'preview' ? value : 'compact';
}

export default function App() {
  const [surface, setSurface] = useState<Surface>(readSurface);

  useEffect(() => {
    const handleNavigation = () => setSurface(readSurface());
    window.addEventListener('popstate', handleNavigation);
    return () => window.removeEventListener('popstate', handleNavigation);
  }, []);

  if (surface === 'history') return <FullHistoryView />;
  if (surface === 'setup') return <SetupView />;
  if (surface === 'preview') return <PreviewGallery />;
  return <CompactView />;
}
