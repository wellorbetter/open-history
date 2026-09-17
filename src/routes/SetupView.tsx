import { EyeOff, HardDrive, KeyboardOff, MicOff, MonitorOff } from 'lucide-react';
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
            title="Stored only here"
            // Not "no cloud processing": asking a coding agent to read a row back sends that row's
            // window titles to the agent's vendor. It takes an explicit click and nothing else in
            // the app leaves the machine, but an unqualified promise here would be contradicted by
            // a feature two screens away.
            text="No account and no telemetry. Nothing leaves this Mac unless you ask an agent to read a row back."
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
          {/* The private-browsing half of this claim was removed: the policy can exclude a private
              context, but nothing ever reports one — there is no browser adapter, and both callers
              pass `private_context: false`. Password-manager exclusion is real and enforced. */}
          Password managers are excluded before anything is written to disk.
        </div>
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
