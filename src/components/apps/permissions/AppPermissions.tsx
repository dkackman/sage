import type { SageAppPermissions } from '@/bindings';
import React, { useEffect, useMemo, useState } from 'react';
import {
  AppPermissionGroup,
  type AppPermissionTreeNode,
} from './AppPermissionGroup';

type Tone = 'default' | 'added' | 'removed' | 'warning';

interface PermissionEntry {
  key: string;
  required: boolean;
  granted: boolean;
}

interface Props {
  permissions: SageAppPermissions | null | undefined;
  grantedPermissions?: string[];
  editable?: boolean;
  tone?: Tone;
  onGrantedPermissionsChange?: React.Dispatch<React.SetStateAction<string[]>>;
}

function buildEntries(
  permissions: SageAppPermissions | null | undefined,
  grantedPermissions: string[],
): PermissionEntry[] {
  const grantedSet = new Set(grantedPermissions);

  const requiredEntries = (permissions?.required ?? []).map((key) => ({
    key,
    required: true,
    granted: grantedSet.has(key),
  }));

  const optionalEntries = (permissions?.optional ?? []).map((key) => ({
    key,
    required: false,
    granted: grantedSet.has(key),
  }));

  return [...requiredEntries, ...optionalEntries].sort((a, b) =>
    a.key.localeCompare(b.key),
  );
}

function buildTree(entries: PermissionEntry[]): AppPermissionTreeNode {
  const root: AppPermissionTreeNode = {
    segment: '',
    fullPath: '',
    children: [],
  };

  for (const entry of entries) {
    const parts = entry.key.split('.');
    let current = root;

    for (let index = 0; index < parts.length; index += 1) {
      const segment = parts[index];
      const fullPath = parts.slice(0, index + 1).join('.');
      const isLeaf = index === parts.length - 1;

      let child = current.children.find((node) => node.segment === segment);

      if (!child) {
        child = {
          segment,
          fullPath,
          children: [],
        };
        current.children.push(child);
      }

      if (isLeaf) {
        child.item = {
          key: entry.key,
          required: entry.required,
          granted: entry.granted,
        };
      }

      current = child;
    }
  }

  currentSort(root);
  return root;
}

function currentSort(node: AppPermissionTreeNode) {
  node.children.sort((a, b) => {
    const aIsGroup = !a.item;
    const bIsGroup = !b.item;

    if (aIsGroup !== bIsGroup) {
      return aIsGroup ? -1 : 1;
    }

    return a.segment.localeCompare(b.segment);
  });

  for (const child of node.children) {
    currentSort(child);
  }
}

export function AppPermissions({
  permissions,
  grantedPermissions = [],
  editable = false,
  tone = 'default',
  onGrantedPermissionsChange,
}: Props) {
  const [localGrantedPermissions, setLocalGrantedPermissions] =
    useState<string[]>(grantedPermissions);

  useEffect(() => {
    setLocalGrantedPermissions(grantedPermissions);
  }, [grantedPermissions]);

  const entries = useMemo(() => {
    return buildEntries(permissions, localGrantedPermissions);
  }, [permissions, localGrantedPermissions]);

  const tree = useMemo(() => buildTree(entries), [entries]);

  const hasEntries = entries.length > 0;

  function handleToggle(fullKey: string, nextGranted: boolean) {
    if (!editable) {
      return;
    }

    setLocalGrantedPermissions((prev) => {
      const prevSet = new Set(prev);

      if (nextGranted) {
        prevSet.add(fullKey);
      } else {
        prevSet.delete(fullKey);
      }

      const requiredKeys = new Set(
        Object.keys(permissions?.required ?? {}),
      );

      for (const key of requiredKeys) {
        prevSet.add(key);
      }

      const next = [...prevSet].sort((a, b) => a.localeCompare(b));

      onGrantedPermissionsChange?.(next);
      return next;
    });
  }

  if (!hasEntries) {
    return (
      <div className='text-sm text-muted-foreground'>
        This app does not request any permissions.
      </div>
    );
  }

  return (
    <AppPermissionGroup
      node={tree}
      editable={editable}
      tone={tone}
      onToggle={handleToggle}
    />
  );
}
