import { Checkbox } from '@/components/ui/checkbox';
import React from 'react';

interface Props {
  label: string;
  fullKey: string;
  required: boolean;
  granted: boolean;
  editable?: boolean;
  tone?: 'default' | 'added' | 'removed' | 'warning';
  onToggle?: (fullKey: string, nextGranted: boolean) => void;
}

export function AppPermissionItem({
  label,
  fullKey,
  required,
  granted,
  editable = false,
  tone = 'default',
  onToggle,
}: Props) {
  const textClassName =
    tone === 'removed'
      ? 'text-destructive line-through'
      : tone === 'warning'
        ? 'text-amber-600'
        : tone === 'added'
          ? 'text-emerald-600'
          : '';

  return (
    <label className='flex items-center gap-3 text-sm'>
      <Checkbox
        checked={granted}
        disabled={!editable || required}
        onCheckedChange={(checked) => {
          onToggle?.(fullKey, Boolean(checked));
        }}
      />

      <span className={textClassName}>
        {label}
        {required ? ' (required)' : ''}
      </span>
    </label>
  );
}
