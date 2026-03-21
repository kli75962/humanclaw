import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Camera, ImagePlus, QrCode } from 'lucide-react';
import { scan, Format } from '@tauri-apps/plugin-barcode-scanner';
import jsQR from 'jsqr';
import '../style/SettingsQrPairing.css';

export function ShowQrView() {
  const [svg, setSvg] = useState('');
  const [allAddresses, setAllAddresses] = useState<string[]>([]);
  const [customAddress, setCustomAddress] = useState('');
  const [error, setError] = useState('');
  const [fetchingPublicIp, setFetchingPublicIp] = useState(false);

  useEffect(() => {
    invoke<string[]>('get_all_local_addresses')
      .then(setAllAddresses)
      .catch(() => setAllAddresses([]));
  }, []);

  useEffect(() => {
    if (allAddresses.length === 0 && !customAddress) return;
    setSvg('');
    setError('');
    const timer = setTimeout(() => {
      const opts = customAddress.trim() ? { customAddress: customAddress.trim() } : {};
      invoke<string>('get_qr_pair_svg', opts)
        .then(setSvg)
        .catch((e) => setError(String(e)));
    }, 400);
    return () => clearTimeout(timer);
  }, [allAddresses, customAddress]);

  async function fetchPublicIp() {
    setFetchingPublicIp(true);
    try {
      const resp = await fetch('https://api.ipify.org?format=text');
      const ip = (await resp.text()).trim();
      const port = allAddresses[0]?.split(':')[1] ?? '9876';
      setCustomAddress(`${ip}:${port}`);
    } catch {
      setError('Could not fetch public IP.');
    } finally {
      setFetchingPublicIp(false);
    }
  }

  return (
    <div className="qr-view">
      {svg ? (
        <div className="qr-code-box" dangerouslySetInnerHTML={{ __html: svg }} />
      ) : error ? (
        <p className="qr-error-text">{error}</p>
      ) : (
        <div className="qr-loading">
          <QrCode size={48} />
        </div>
      )}

      <div className="qr-addresses">
        <p className="qr-addresses-label">Detected addresses ({allAddresses.length})</p>
        {allAddresses.length > 0 && (
          <div className="qr-chip-row">
            {allAddresses.map((addr) => (
              <span key={addr} className="qr-chip">{addr}</span>
            ))}
          </div>
        )}
        <p className="qr-hint">All addresses above are encoded in the QR. Phone will try each automatically.</p>
        <div className="qr-custom-row">
          <input
            value={customAddress}
            onChange={(e) => setCustomAddress(e.target.value)}
            placeholder="Override with custom ip:port"
            className="qr-custom-input"
          />
          <button onClick={fetchPublicIp} disabled={fetchingPublicIp} className="qr-public-ip-btn">
            {fetchingPublicIp ? '…' : 'Public IP'}
          </button>
        </div>
        <p className="qr-hint">
          On mobile data? Click <strong>Public IP</strong> and ensure port forwarding is set up on your router.
        </p>
      </div>
    </div>
  );
}

export function ScanView({ onPaired }: { onPaired: () => void; isAndroid: boolean }) {
  const [status, setStatus] = useState<'idle' | 'scanning' | 'pairing' | 'done' | 'error'>('idle');
  const [error, setError] = useState('');
  const fileInputRef = useRef<HTMLInputElement>(null);

  async function pairWithPayload(raw: string) {
    let parsed: { addresses?: string[]; address?: string; pairing_token?: string; hash_key?: string };
    try {
      parsed = JSON.parse(raw);
    } catch {
      throw new Error('Invalid QR code — not a valid pairing code.');
    }
    const token = parsed.pairing_token ?? parsed.hash_key ?? null;
    const addresses = parsed.addresses ?? (parsed.address ? [parsed.address] : []);
    if (addresses.length === 0 || !token) {
      throw new Error('Invalid QR code — missing address or pairing token.');
    }
    setStatus('pairing');
    await invoke('pair_from_qr', { addresses, hashKey: token });
    setStatus('done');
    onPaired();
  }

  async function handleImageFile(file: File) {
    setStatus('scanning');
    setError('');
    try {
      const bitmap = await createImageBitmap(file);
      const canvas = document.createElement('canvas');
      canvas.width = bitmap.width;
      canvas.height = bitmap.height;
      const ctx = canvas.getContext('2d')!;
      ctx.drawImage(bitmap, 0, 0);
      const imageData = ctx.getImageData(0, 0, bitmap.width, bitmap.height);
      const code = jsQR(imageData.data, imageData.width, imageData.height);
      if (!code) throw new Error('No QR code detected in image.');
      await pairWithPayload(code.data);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setStatus('error');
    }
  }

  async function handleScan() {
    setStatus('scanning');
    setError('');
    try {
      const result = await scan({ formats: [Format.QRCode], windowed: true });
      await pairWithPayload(result.content);
    } catch (e) {
      const msg = e instanceof Error ? e.message : typeof e === 'object' ? JSON.stringify(e) : String(e);
      const normalized = msg.toLowerCase();
      if (normalized.includes('cancel') || normalized.includes('closed') || normalized.includes('dismiss')) {
        setStatus('idle');
        setError('');
        return;
      }
      setError(msg);
      setStatus('error');
    }
  }

  return (
    <div className="scan-view">
      <input
        ref={fileInputRef}
        type="file"
        accept="image/*"
        style={{ display: 'none' }}
        onChange={(e) => { const f = e.target.files?.[0]; if (f) handleImageFile(f); e.target.value = ''; }}
      />

      {status === 'idle' && (
        <div className="scan-idle">
          <button onClick={handleScan} className="scan-btn--primary">
            <Camera size={16} />
            Scan QR Code
          </button>
          <button onClick={() => fileInputRef.current?.click()} className="scan-btn--secondary">
            <ImagePlus size={15} />
            Import from image
          </button>
        </div>
      )}

      {status === 'scanning' && <p className="scan-status">Opening camera…</p>}
      {status === 'pairing' && <p className="scan-status">Linking device…</p>}

      {status === 'error' && (
        <div className="scan-error">
          <p className="scan-error-msg">❌ {error}</p>
          <div className="scan-retry-row">
            <button onClick={handleScan} className="scan-btn--secondary">
              <Camera size={14} />
              Try again
            </button>
            <button onClick={() => fileInputRef.current?.click()} className="scan-btn--secondary">
              <ImagePlus size={14} />
              Image
            </button>
          </div>
        </div>
      )}

      <p className="scan-hint">Scan live or import a screenshot of the QR code.</p>
    </div>
  );
}
