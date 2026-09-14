import { ArrowRight, EyeOff, HardDrive, KeyboardOff, MicOff, MonitorOff } from 'lucide-react';
import { BrandMark } from '../components/BrandMark';

export function SetupView() {
  return (
    <main className="setup-shell">
      <section className="setup-card">
        <BrandMark size={48} />
        <p className="eyebrow">Welcome to OpenHistory</p>
        <h1>Remember the work, not the surveillance.</h1>
        <p className="setup-lead">
          OpenHistory turns consented semantic activity into a private, resumable timeline stored on
          this device.
        </p>
        <div className="privacy-promise">
          <Promise
            icon={<HardDrive />}
            title="Local only"
            text="No account, cloud processing, or telemetry."
          />
          <Promise icon={<MonitorOff />} title="No screenshots" text="Pixels are never captured." />
          <Promise icon={<MicOff />} title="No audio" text="Microphones are never accessed." />
          <Promise
            icon={<KeyboardOff />}
            title="No raw keys"
            text="Typed key sequences are not stored."
          />
        </div>
        <div className="setup-note">
          <EyeOff size={17} aria-hidden="true" />
          Password managers and private browser contexts are excluded before persistence.
        </div>
        <button className="primary-button" type="button">
          Continue to permission setup <ArrowRight size={17} aria-hidden="true" />
        </button>
      </section>
    </main>
  );
}

function Promise({ icon, title, text }: { icon: React.ReactElement; title: string; text: string }) {
  return (
    <div className="promise-item">
      <span aria-hidden="true">{icon}</span>
      <div>
        <strong>{title}</strong>
        <small>{text}</small>
      </div>
    </div>
  );
}
