import React from 'react';
import { AppPermissionItem } from './AppPermissionItem';

export interface AppPermissionTreeNode {
  segment: string;
  fullPath: string;
  children: AppPermissionTreeNode[];
  item?: {
    key: string;
    required: boolean;
    granted: boolean;
  };
}

interface Props {
  node: AppPermissionTreeNode;
  editable?: boolean;
  tone?: 'default' | 'added' | 'removed' | 'warning';
  onToggle?: (fullKey: string, nextGranted: boolean) => void;
}

function segmentLabel(segment: string): string {
  return segment
    .split('_')
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(' ');
}

export function AppPermissionGroup({
  node,
  editable = false,
  tone = 'default',
  onToggle,
}: Props) {
  if (node.item) {
    return (
      <AppPermissionItem
        label={segmentLabel(node.segment)}
        fullKey={node.item.key}
        required={node.item.required}
        granted={node.item.granted}
        editable={editable}
        tone={tone}
        onToggle={onToggle}
      />
    );
  }

  const label = node.fullPath ? segmentLabel(node.segment) : 'Permissions';

  return (
    <div className='space-y-2'>
      {node.fullPath ? (
        <div className='text-sm font-medium'>{label}</div>
      ) : null}

      <div
        className={
          node.fullPath ? 'space-y-2 rounded-md border p-3' : 'space-y-4'
        }
      >
        {node.children.map((child) => (
          <AppPermissionGroup
            key={child.fullPath}
            node={child}
            editable={editable}
            tone={tone}
            onToggle={onToggle}
          />
        ))}
      </div>
    </div>
  );
}
