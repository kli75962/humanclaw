import { useEffect, useState } from 'react';
import { X } from 'lucide-react';
import '../../style/PersonaBuildNotice.css';

export type PersonaBuildNoticeStatus = 'creating' | 'done' | 'interrupted';

interface Props {
  status: PersonaBuildNoticeStatus;
  displayName: string;
  onClose: () => void;
}

export function PersonaBuildNotice({ status, displayName, onClose }: Props) {
  const [dots, setDots] = useState('');

  useEffect(() => {
    if (status !== 'creating') { setDots(''); return; }
    const id = setInterval(() => setDots((d) => (d.length >= 3 ? '' : d + '.')), 500);
    return () => clearInterval(id);
  }, [status]);

  return (
    <div className={`persona-notice persona-notice--${status}`}>
      <button className="persona-notice-close" onClick={onClose} aria-label="Close">
        <X size={13} />
      </button>
      <p className="persona-notice-msg">
        {status === 'creating' && <>Creating{dots}</>}
        {status === 'done' && `Successful create persona - ${displayName}`}
        {status === 'interrupted' &&
          'Something interrupted the process, will continue the process when back to normal'}
      </p>
    </div>
  );
}
