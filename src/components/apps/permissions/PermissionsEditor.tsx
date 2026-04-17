import { useState } from 'react';
import { Button } from '@/components/ui/button';

interface Props {
  app: {
    grantedPermissions: string[];
  };
  onCancel: () => void;
  onApply: (permissions: string[]) => void;
}

const ALL_PERMISSIONS = [
  'persistent_storage',
  // add more later
];

export function PermissionsEditor({ app, onCancel, onApply }: Props) {
  const [selected, setSelected] = useState<Set<string>>(
    new Set(app.grantedPermissions),
  );

  function toggle(p: string) {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(p)) {
        next.delete(p);
      } else {
        next.add(p);
      }
      return next;
    });
  }

  return (
    <div className='space-y-4'>
      <div className='space-y-2'>
        {ALL_PERMISSIONS.map((p) => (
          <label
            key={p}
            className='flex items-center gap-2 text-sm cursor-pointer'
          >
            <input
              type='checkbox'
              checked={selected.has(p)}
              onChange={() => toggle(p)}
            />
            {p}
          </label>
        ))}
      </div>

      <div className='flex justify-end gap-2'>
        <Button variant='outline' onClick={onCancel}>
          Cancel
        </Button>
        <Button
          onClick={() => {
            onApply(Array.from(selected));
          }}
        >
          Apply
        </Button>
      </div>
    </div>
  );
}
