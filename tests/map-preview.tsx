// Visual fixture: real RadarMap, no authentication or changes to application data.
import ReactDOM from 'react-dom/client';
import { RadarMap } from '../src/RadarMap';
import { defaults, emptySnapshot } from '../src/api';
import '../src/styles.css';

ReactDOM.createRoot(document.getElementById('root')!).render(
  <div style={{ height: '100vh', display: 'flex', flexDirection: 'column' }}>
    <div style={{ padding: 12 }}>
      Проверка офлайн-подложки · реальные данные OSM · без симуляции
    </div>
    <RadarMap
      snapshot={{ ...emptySnapshot, sweepBearing: 1.2 }}
      settings={defaults}
      zones={[]}
      drawing={false}
      draft={[]}
      onPoint={() => {}}
      onIdentify={() => {}}
      onHome={() => {}}
    />
  </div>,
);
